# 52-util-app — Rust design

Target: Linux, KDE Plasma, GTK 4.16 through `gtk4-rs` 0.11 (`gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, as in `/home/blackbox/kurobayomi/Cargo.toml`), no libadwaita. The mockup is a single crate named `kurobayomi` with `src/{main.rs, ui.rs, screens.rs, spec.rs, settings.rs, entry.rs, player.rs, clamp.rs}` and JSON specs in `spec/`.

This design covers the util surface of Aniyomi (40 files, see `p1-inventory.md`). It splits it into three layers so that the logic is testable without GTK.

---

## 1. Crate and module layout

```
kurobayomi/                       (workspace root = /home/blackbox/kurobayomi)
├── kuro-core/                    pure logic, NO gtk/gio/glib dependency
│   └── src/
│       ├── lib.rs
│       ├── util/
│       │   ├── mod.rs
│       │   ├── size.rs           format_bytes                     (StorageUtil.kt toSize)
│       │   ├── collections.rs    insert_separators(+_reversed)    (CollectionUtils.kt)
│       │   └── text.rs           truncate_center                  (dependency outside scope; see behaviour 9.6)
│       ├── selection.rs          next_unread / next_unseen / dedupe / filter_downloaded
│       │                          (ChapterGetNextUnread, EpsiodeGetNextUnseen, ChapterRemoveDuplicates,
│       │                           ChapterFilterDownloaded, EpisodeFilterDownloaded)
│       ├── sort.rs               chapter/episode comparators      (ChapterSorter.kt, EpisodeSorter.kt)
│       ├── covers.rs             prep_update_cover / remove / edit decisions (MangaExtensions.kt, AnimeExtensions.kt)
│       ├── airing.rs             AniList/MAL/Simkl parsing + flag packing (AniChartApi.kt)
│       ├── time/
│       │   ├── mod.rs
│       │   ├── relative.rs       RelativeTime decision (DateExtensions.kt relative functions)
│       │   └── format.rs         absolute formats via icu_datetime (DateExtensions.kt)
│       ├── locale.rs             display names, comparator, sentinels (LocaleHelper.kt)
│       ├── geometry.rs           NormRect::invert, visibility rule (RectFExtensions.kt, ViewExtensions.kt)
│       ├── pkce.rs               code verifier                    (PkceUtil.kt)
│       ├── crashlog.rs           report text builder              (CrashLogUtil.kt)
│       ├── reader_theme.rs       background choice                (ContextExtensions.kt createReaderThemeContext)
│       ├── layout.rs             tablet/compact breakpoints       (DisplayExtensions.kt)
│       └── build_info.rs         UPDATER_ENABLED, IS_DEBUG_BUILD, IS_RELEASE_BUILD (BuildConfig.kt)
├── kuro-platform/                gio, ashpd, zbus, PAM wrappers; NO widgets
│   └── src/
│       ├── lib.rs
│       ├── files.rs              save_stream, dir_size, cache_file (OkioExtensions, StorageUtil.UniFile.size, ContextExtensions.createFileInCacheDir/getUriSize)
│       ├── open.rs               open_uri, reveal_file, save_dialog helpers (ContextExtensions.openInBrowser, IntentExtensions.toShareIntent)
│       ├── notify.rs             Notifier over gio::Notification  (NotificationExtensions.kt)
│       ├── net.rs                NetworkState, watch_network      (NetworkStateTracker.kt, NetworkExtensions.kt)
│       ├── appearance.rs         color_scheme(), animation_scale() (DisplayExtensions.isNightMode, AnimationExtensions.kt)
│       ├── auth.rs               pam_authenticate                 (AuthenticatorUtil.kt)
│       ├── jobs.rs               JobRegistry                      (WorkManagerExtensions.kt)
│       └── paths.rs              XDG directories
├── kuro-i18n/                    MR/AYMR key table (generated from base strings.xml), plural rendering via icu_plurals,
│                                  Strings lookup used by kuro-core::locale and time::relative
└── kurobayomi/ (the existing binary crate, src/)
    └── src/
        ├── main.rs               starts tokio runtime, glib main loop, prefs, builds window (existing build_ui at main.rs:22)
        ├── ui/toast.rs           Toaster (replaces Toast in player.rs:39 "pill")    (ContextExtensions.toast call sites)
        ├── ui/checkbox.rs        tri-state CheckButton                          (CheckboxState.kt)
        ├── ui/popover.rs         PopoverMenu from a gio::Menu                    (ViewExtensions.popupMenu; ui.rs:202 menu_button today)
        ├── ui/bind.rs            Pref<T> <-> widget binding                      (PreferenceMutableState.kt)
        └── ui/layout.rs          window-width breakpoints -> Layout              (DisplayExtensions.prepareTabletUiContext)
```

Dependency direction: `kurobayomi` -> `kuro-platform` -> `kuro-core`. `kuro-core` never depends on `gtk`, `gio` or `glib`. `kuro-platform` depends on `gio` (through `gtk` re-export of `gio`) and `ashpd`/`zbus`, but has no widgets.

Cargo features to add to the mockup (`Cargo.toml`): `tokio` (rt-multi-thread, macros, sync, time, fs, io-util), `reqwest` (rustls-tls, json), `serde` (already), `serde_json` (already), `chrono`, `icu_displaynames`, `icu_datetime`, `icu_plurals`, `icu_collator` (ICU4X 2.x series), `base64`, `rand`, `md5`, `zbus` (tokio feature), `ashpd` (default features off, tokio), `pam` (feature-gated `auth`), `thiserror`, `async-channel`, `walkdir` is NOT needed (manual recursion with `symlink_metadata`).

---

## 2. Key types and signatures

### 2.1 `kuro-core::util::size`
```rust
/// StorageUtil.kt `Long.toSize()`. Decimal units, see behaviour section 9.2.
pub fn format_bytes(n: i64) -> String;
```

### 2.2 `kuro-core::util::collections`
```rust
/// CollectionUtils.kt `insertSeparators`. Calls `gen` exactly size+1 times
/// (before = None for the first call, after = None for the last).
pub fn insert_separators<T, R>(
    items: &[T],
    mut gen: impl FnMut(Option<&T>, Option<&T>) -> Option<R>,
    wrap: impl Fn(&T) -> R,
) -> Vec<R>;

/// CollectionUtils.kt `insertSeparatorsReversed`. Same output as insert_separators
/// for pure generators; the generator is called in reverse index order (upstream parity).
pub fn insert_separators_reversed<T, R>(
    items: &[T],
    gen: impl FnMut(Option<&T>, Option<&T>) -> Option<R>,
    wrap: impl Fn(&T) -> R,
) -> Vec<R>;
```
(`wrap` is the Rust form of `T : R` upper bound: Kotlin allows an item to be its own row type, Rust needs an explicit conversion.)

### 2.3 `kuro-core::selection`
```rust
/// Read-only view of a chapter or an episode. Implemented by the domain structs.
pub trait Unit: Clone {
    fn id(&self) -> i64;
    fn number(&self) -> f64;            // chapterNumber / episodeNumber
    fn name(&self) -> &str;
    fn scanlator(&self) -> Option<&str>;
    fn is_read(&self) -> bool;          // Chapter.read / Episode.seen
    fn source_order(&self) -> i64;
    fn date_upload(&self) -> i64;
}

/// View filters applied before the search (ChapterFilter.kt / EpisodeFilter.kt).
#[derive(Clone, Copy)]
pub enum Tri { Disabled, EnabledIs, EnabledNot }

pub struct ViewFilter {
    pub unread: Tri,
    pub bookmarked: Tri,
    pub downloaded: Tri,        // already resolved, incl. downloaded-only override (domain Manga.downloadedFilter)
}

/// ChapterGetNextUnread.kt / EpsiodeGetNextUnseen.kt. `sort_desc` = sortDescending().
pub fn next_unread<U: Unit>(
    units: &[U],
    filter: &ViewFilter,
    sorted: impl Fn(&[U]) -> Vec<U>,     // applies getChapterSort / getEpisodeSort
    sort_desc: bool,
    is_downloaded: impl Fn(&U) -> bool,
    is_local: bool,
) -> Option<U>;

/// ChapterRemoveDuplicates.kt
pub fn dedupe<U: Unit>(units: &[U], current: &U) -> Vec<U>;

/// ChapterFilterDownloaded.kt / EpisodeFilterDownloaded.kt
pub fn filter_downloaded<U: Unit>(units: &[U], is_local: bool, is_downloaded: impl Fn(&U) -> bool) -> Vec<U>;
```
Notes
- `next_unread` returns the first unread (ascending) or the last unread (descending) after filters and sort: the same logic as `ChapterGetNextUnread.kt:14-18`.
- `dedupe` keys by `number().to_bits()` after normalising `-0.0` to `0.0`. Output keeps the first-appearance group order.

### 2.4 `kuro-core::sort`
```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChapterSorting { Source = 0x0, Number = 0x100, UploadDate = 0x200, Alphabet = 0x300 }
pub fn chapter_sort_key(bits: i64) -> Result<ChapterSorting, SortError>; // mask 0x300; SortError = "NotImplementedError" equivalent
pub fn compare_chapters(a: &impl Unit, b: &impl Unit, s: ChapterSorting, desc: bool, collate: &Collator) -> Ordering;
```
The `Source` + `desc` branch is inverted upstream (`ChapterSorter.kt:11-13`): `compare_chapters` reproduces it on purpose (test in p1-edge-cases-and-tests.md, case SORT-02). Same for episodes with `EpisodeSorting`.

### 2.5 `kuro-core::covers`
```rust
pub struct CoverState<'a> { pub is_local: bool, pub has_custom_cover: bool, pub thumbnail_url: Option<&'a str> }
pub enum CoverDecision {
    /// No change at all.
    Keep,
    /// Local source: bump coverLastModified only.
    BumpOnly { last_modified_ms: i64 },
    /// Remote, custom cover exists: delete the OLD thumbnail file, keep entry unchanged.
    DeleteOldThumbOnly,
    /// Remote, no custom cover: delete old thumbnail file and bump.
    DeleteOldThumbAndBump { last_modified_ms: i64 },
}
/// MangaExtensions.kt prepUpdateCover, pure part. `remote_thumb` = SManga.thumbnail_url.
pub fn prep_update_cover(cur: &CoverState, remote_thumb: Option<&str>, refresh_same_url: bool, now_ms: i64) -> CoverDecision;
```

### 2.6 `kuro-core::airing`
```rust
pub const AIRING_DEFAULT: (i32, i64) = (1, 0);

pub struct AiringTrack { pub tracker: Tracker, pub remote_id: i64 }
#[derive(Clone, Copy)] pub enum Tracker { AniList, MyAnimeList, Simkl, Other }

/// Pure selection rule: first track with a tracker in {AniList, MAL, Simkl}.
pub fn pick_track(tracks: &[AiringTrack]) -> Option<&AiringTrack>;

/// Stored-pair decoding from viewer flags (Anime.kt:79-83).
pub fn decode_airing(viewer_flags: i64) -> (i32, i64);
/// Encoding for SetAnimeViewerFlags (SetAnimeViewerFlags.kt:30-47). Truncates silently like upstream.
pub fn encode_airing(viewer_flags: i64, episode: i32, airing_at_s: i64) -> i64;

/// Simkl calendar entry (serde). Fields exactly as observed in section 6.2 of p1-data-and-config.md.
#[derive(serde::Deserialize)]
pub struct SimklEntry {
    pub date: String,                 // ISO-8601 with offset
    pub ids: SimklIds,
    pub episode: SimklEpisode,
}
#[derive(serde::Deserialize)]
pub struct SimklIds { pub simkl_id: i64, pub mal: Option<String> }
#[derive(serde::Deserialize)]
pub struct SimklEpisode { pub episode: i32 }

/// Earliest entry for `simkl_id` whose date is strictly after `now`. Returns None when absent.
pub fn next_simkl_airing(entries: &[SimklEntry], simkl_id: i64, now_s: i64) -> Option<(i32, i64, Option<i64>)>;
// returns (episode, airing_at_seconds, mal_id_if_present)
```

`kuro-net::airing` (HTTP, area 12 client): `async fn load_airing(client: &reqwest::Client, anime: &AnimeAiringInput, tracks: &[AiringTrack], manual: bool) -> (i32, i64)` with the upstream order and all `(1, 0)` fallbacks (behaviour section 7). Errors are logged and converted to `AIRING_DEFAULT` at this boundary.

### 2.7 `kuro-core::time`
```rust
pub enum RelativeTime {
    Today,
    Days { n: i64, upcoming: bool },        // "Yesterday"/"%d days ago"/"In %d days"/"Tomorrow"
    Hours { n: i64, upcoming: bool },
    Minutes { n: i64, upcoming: bool },
    Now,
    Absolute,                               // caller formats with the date formatter
}
/// LocalDate variant (behaviour 10.2). Dates in local calendar terms.
pub fn relative_date(date: chrono::NaiveDate, today: chrono::NaiveDate) -> RelativeTime;
/// LocalDateTime variant (behaviour 10.3). `now` and `release` are naive local times.
pub fn relative_datetime(release: chrono::NaiveDateTime, now: chrono::NaiveDateTime) -> RelativeTime;
```
Rendering (`kuro-i18n`): `fn render(rt: RelativeTime, locale: &Locale) -> String` using `icu_plurals::PluralRules::try_new_cardinal` to pick `one`/`other`. Keys: `relative_time`, `upcoming_relative_time`, `relative_time_today`, `relative_time_now`, `relative_time_hours`, `relative_time_minutes`, `upcoming_relative_time_hours`, `upcoming_relative_time_minutes` (behaviour 10.1).

`time::format`: `pub fn short_date(dt: NaiveDate, locale: &Locale) -> String` and `short_time(...)`, `short_datetime` = `"{date} {time}"`. Uses `icu_datetime::DateTimeFormatter` with `Length::Short`.

Zones: `pub fn convert_epoch_millis_zone(ms: i64, from: chrono_tz::Tz, to: chrono_tz::Tz) -> i64` with `chrono-tz` (used only by tracker dialogs, area 27).

### 2.8 `kuro-core::locale`
```rust
pub const PINNED_KEY: &str = "pinned";
pub const LAST_USED_KEY: &str = "last_used";

pub struct LocaleNames { cache: std::sync::Mutex<HashMap<String, String>> }
impl LocaleNames {
    pub fn localized_display_name(&self, lang: Option<&str>) -> String;   // getLocalizedDisplayName
    pub fn display_name(&self, lang: &str) -> String;                    // getDisplayName (English display locale)
    pub fn source_display_name(&self, lang: Option<&str>, strings: &Strings) -> String; // getSourceDisplayName
    pub fn compare(&self, a: &str, b: &str) -> Ordering;                 // comparator: "all" first
}
pub fn default_enabled_languages(system_lang: Option<&str>) -> BTreeSet<String>; // getDefaultEnabledLanguages
pub fn system_language() -> String;   // glib::language_names() first entry
```
`Strings` is the i18n lookup (`kuro-i18n`) providing `last_used_source`, `pinned_sources`, `other_source`, `multi_lang`.

### 2.9 `kuro-core::geometry`
```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormRect { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapInvert { None, Horizontal, Vertical, Both }
impl NormRect { pub fn invert(self, mode: TapInvert) -> NormRect; }

/// ViewExtensions.kt isVisibleOnScreen: positive-area intersection only.
pub fn intersects_positive_area(a: Rect2, b: Rect2) -> bool;
pub struct Rect2 { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }
```

### 2.10 `kuro-core::pkce`
```rust
/// 50 random bytes, base64url without padding (67 chars). PkceUtil.kt.
pub fn generate_code_verifier() -> String;
```
Uses `rand::rngs::OsRng` and `base64::engine::general_purpose::URL_SAFE_NO_PAD`.

### 2.11 `kuro-core::crashlog`
```rust
pub struct DebugInfo { pub app_id: &'static str, pub app_version: String, pub commit: String, pub build_time: String,
                       pub os: String, pub kernel: String, pub session: String, pub hardware: String,
                       pub gtk: String, pub glib: String, pub now: String, pub mpv: String,
                       pub libplacebo: String, pub ffmpeg: String }
pub struct ExtensionInfo { pub name: String, pub pkg: String, pub installed_version: String, pub available_version: Option<String>,
                           pub installed_code: i64, pub available_code: Option<i64>, pub obsolete: bool }
pub fn debug_info_text(i: &DebugInfo) -> String;                    // getDebugInfo, labels verbatim
pub fn problematic_extensions(exts: &[ExtensionInfo]) -> Option<String>; // getMangaExtensionsInfo / getAnimeExtensionsInfo
pub fn build_report(info: &DebugInfo, exts: &[ExtensionInfo], panic_or_error: Option<&str>, log_tail: &str) -> String;
```

### 2.12 `kuro-core::reader_theme`, `kuro-core::layout`, `kuro-core::build`
```rust
pub enum ReaderBackground { Light, Dark }
pub fn reader_background(theme: i32, theme_mode: ThemeMode, system_dark: bool) -> ReaderBackground;

pub enum Layout { Compact, Wide }
pub fn layout_for(mode: TabletUiMode, width_px: i32, height_px: i32) -> Layout;

pub const UPDATER_ENABLED: bool = false;
pub const IS_DEBUG_BUILD: bool = cfg!(debug_assertions);
pub const IS_RELEASE_BUILD: bool = !cfg!(debug_assertions);
```

### 2.13 `kuro-platform::files`
```rust
/// OkioExtensions.kt saveTo(file). Writes `<dest>.part`, renames on success, removes part on error.
pub async fn save_stream<R: tokio::io::AsyncRead + Unpin>(src: R, dest: &Path) -> io::Result<()>;

/// StorageUtil.kt UniFile.size(). Iterative, does not follow symlinks.
pub fn dir_size(path: &Path) -> io::Result<u64>;

/// ContextExtensions.createFileInCacheDir: delete existing, create empty file under the cache dir.
pub fn create_cache_file(name: &str) -> io::Result<PathBuf>;

/// ContextExtensions.getUriSize for file:// URIs only.
pub fn file_size(uri: &str) -> Option<u64>;
```

### 2.14 `kuro-platform::open`
```rust
/// ContextExtensions.openInBrowser. Uses gtk::UriLauncher (portal aware). Error = display text.
pub fn open_url(parent: Option<&gtk::Window>, url: &str) -> Result<(), glib::Error>;
pub fn reveal_path(parent: Option<&gtk::Window>, path: &Path);
/// IntentExtensions.toShareIntent replacement: opens a native save dialog. Returns None on cancel.
pub async fn save_dialog(parent: &gtk::Window, suggested_name: &str) -> Option<PathBuf>;
```

### 2.15 `kuro-platform::notify`
```rust
pub enum NotifyChannel { Library, Downloads, Updates, Backup, Other }
pub struct Notifier { app: gtk::Application }
impl Notifier {
    pub fn post(&self, id: u32, channel: NotifyChannel, title: &str, body: Option<&str>, icon: Option<&gio::Icon>);
    pub fn cancel(&self, id: u32);
    pub fn post_batch(&self, items: &[(u32, NotifyChannel, String)]);   // notify(List) equivalent
}
```
Priority map: `Library`/`Other` -> `Normal`, `Downloads`/`Backup` -> `Low`, `Updates` -> `Normal`. Priority per Android importance table in behaviour 15.2 (importance remapped by the caller).

### 2.16 `kuro-platform::net`
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkState { pub is_connected: bool, pub is_validated: bool, pub is_wifi: bool }
impl NetworkState { pub fn is_online(&self) -> bool { self.is_connected && self.is_validated } }

/// Emits the current state immediately, then on every change.
pub fn watch_network(rt: &tokio::runtime::Handle) -> tokio::sync::watch::Receiver<NetworkState>;
/// NetworkExtensions.isOnline: gio::NetworkMonitor connectivity == Full.
pub fn is_online() -> bool;
/// NetworkExtensions.isConnectedToWifi: NetworkManager PrimaryConnectionType == 802-11-wireless.
pub async fn is_connected_to_wifi() -> bool;
```

### 2.17 `kuro-platform::appearance`
```rust
pub enum ColorScheme { NoPreference, PreferDark, PreferLight }
pub async fn color_scheme() -> ColorScheme;     // ashpd Settings::read("org.freedesktop.appearance","color-scheme")
pub fn animation_scale() -> f32;                // 0.0 when gtk-enable-animations=false, else KDE AnimationDurationFactor (default 1.0), clamped 0..=10
```

### 2.18 `kuro-platform::auth`
```rust
#[derive(Debug, thiserror::Error)]
pub enum AuthError { #[error("{0}")] Pam(String), #[error("unsupported")] Unsupported }
/// AuthenticatorUtil.authenticate. Runs PAM on a blocking thread.
pub async fn authenticate(user: String, password: String) -> Result<(), AuthError>;
pub fn is_authentication_supported() -> bool;   // PAM service file present
```

### 2.19 `kuro-platform::jobs`
```rust
pub struct JobRegistry { inner: std::sync::Mutex<HashMap<&'static str, JobEntry>> }
struct JobEntry { handle: tokio::task::JoinHandle<()>, cancel: tokio_util::sync::CancellationToken }
impl JobRegistry {
    pub fn is_running(&self, tag: &'static str) -> bool;                  // WorkManagerExtensions.isRunning
    pub fn start<F>(&self, tag: &'static str, fut: F) -> bool where F: Future<Output = ()> + Send + 'static; // KEEP policy
    pub fn cancel(&self, tag: &'static str);
}
```
`tokio-util` for `CancellationToken` (add dependency).

---

## 3. Async and threading model

- The GTK main loop owns the main thread (`glib::MainContext::default()`). Widgets are created and touched only there.
- A multi-thread tokio runtime is created once in `main.rs` on a separate thread (`tokio::runtime::Builder::new_multi_thread().enable_all()`). A `tokio::runtime::Handle` is passed to `kuro-platform` services.
- Crossing back to the UI: `async_channel::Sender` in tokio tasks, `glib::spawn_future_local` receives on the main thread. Do not use `glib::MainContext::channel` (deprecated in glib 0.20+).
- Blocking work (file copy is async; `dir_size` and PAM are blocking): `tokio::task::spawn_blocking`.
- Upstream dispatchers mapping (from `CoroutinesExtensions.kt`):
  - `withIOContext` -> `tokio::spawn` / `spawn_blocking`
  - `withUIContext` -> `glib::spawn_future_local` (main thread)
  - `withNonCancellableContext` -> `tokio::spawn` detached (not tied to a window or scope handle; the JoinHandle is dropped on purpose)
- Cancellation: `JobRegistry::cancel` for jobs; window close does not cancel crash dumps.
- Locks: `std::sync::Mutex` only around short map operations, never held across `.await`.
- `OnceLock` for the ICU collator and `LocaleNames` cache.

---

## 4. Error handling

- Library crates: `thiserror` enums per module (`AiringError`, `SortError`, `AuthError`, `io::Error` passthrough). No `anyhow` in libraries.
- Binary (`kurobayomi`): `anyhow::Result` in `main` only.
- Upstream "swallow and default" sites are reproduced explicitly at the boundary (never silently):
  - Airing network failure -> `AIRING_DEFAULT` (`airing::load` boundary).
  - `isOnline` failure -> `false`.
  - `dir_size` per-child error -> that child counts 0 (log at `debug`).
  - Notification post failure -> log `warn`, continue.
  - Clipboard failure -> toast `clipboard_copy_error`.
- Logging: `tracing` with `tracing-subscriber`; crash logs use the in-process ring buffer (last 500 lines).

---

## 5. Integration with the existing mockup

The mockup already has the shapes this design needs. Mapping:

| Mockup location | Current behaviour | Target after this area |
|---|---|---|
| `src/main.rs:22` `build_ui(app)` | builds the window | calls `kuro_platform::paths::init()`, creates the tokio runtime, the `Notifier`, the `JobRegistry`, and passes `Layout` source to the window |
| `src/main.rs:68-70` `connect_default_width_notify` / `connect_maximized_notify` | re-evaluates layout on resize | calls `ui::layout::apply(&window, mode)` (section 2.12) |
| `src/ui.rs:202` `menu_button(icon, entries, on)` | `gtk::Popover` with plain buttons | becomes `ui::popover::from_menu(gio::Menu)`; keep the signature for existing callers |
| `src/ui.rs:221` `item_row` / `Item::Switch` | spec-driven rows | keep; `Item::Tristate` (`spec.rs:8`) uses `ui::checkbox::tri_state()` |
| `src/player.rs:39` `pill(overlay, text)` "Flash a short message centered over overlay (Aniyomi's toast / player update pill)" | in-overlay pill | becomes `ui::toast::Toaster::show(text, Duration)` (used everywhere, not only the player) |
| `src/entry.rs:150` menu with "Copy to clipboard" | no-op | `kuro_platform::clipboard::copy` + toast |
| `src/settings.rs:158` `icon_button("edit-copy-symbolic", "Copy to clipboard")` | no-op | same |
| `src/screens.rs:468` `storage()` | static storage screen | uses `kuro_platform::files::dir_size` and `kuro_core::util::size::format_bytes` |
| `src/screens.rs:354` `downloads()` | static | `JobRegistry::is_running("downloads")` for the busy indicator |
| `src/settings.rs:223` `clear_db(id)` | sample action | unchanged |
| `spec/settings.json` item `{"type":"tristate"}` | JSON spec | rendered via `ui::checkbox` |
| `spec/SCHEMA.md` `"type":"switch"` etc. | schema | no change; the spec rules say strings must be the real English text (`strings.xml`), so every literal in section 2 of behaviour that is shown to the user must use its MR/AYMR key text |

Rule (from `spec/SCHEMA.md`): strings are real English from `strings.xml`. The `kuro-i18n` crate loads the base English file once at build time through `include_str!` of a generated map (Proposal) so the mockup and the app share one source.

---

## 6. Android API to Linux mapping (every API touched in scope)

| Android API / library (scope file) | Linux / Rust replacement | Notes |
|---|---|---|
| `ClipboardManager.setPrimaryClip(ClipData.newPlainText)` (ContextExtensions) | `gdk::Display::clipboard().set_text()` | needs a display and focus on Wayland |
| `Intent.ACTION_VIEW` + `startActivity` (openInBrowser) | `gtk::UriLauncher` (portal `OpenURI`) | `forceDefaultBrowser` dropped |
| `Intent.ACTION_SEND` + chooser (toShareIntent, CrashLogUtil) | `gtk::FileDialog::save` / `gtk::FileLauncher` | no share sheet on GTK |
| `FileProvider` `getUriCompat` (CrashLogUtil) | `gio::File::for_path` | no provider needed |
| `externalCacheDir` (createFileInCacheDir) | `glib::user_cache_dir()/kurobayomi` | |
| `Context.getSystemService(ConnectivityManager)`, `activeNetwork`, `NetworkCapabilities` (NetworkExtensions, NetworkStateTracker) | `gio::NetworkMonitor` + NetworkManager D-Bus (`zbus`) | section 2.16 |
| `WifiManager.isWifiEnabled`, `connectionInfo.bssid` | NetworkManager `PrimaryConnectionType`, `WirelessEnabled` | |
| `ConnectivityManager.registerDefaultNetworkCallback` (networkStateFlow) | `gio::NetworkMonitor::network-changed` + NM `StateChanged` | |
| `NotificationManagerCompat.notify`, `NotificationCompat.Builder`, `NotificationChannelCompat`, `NotificationChannelGroupCompat` (NotificationExtensions) | `gtk::Application::send_notification` / `withdraw_notification` with `gio::Notification` (priority) | channel groups dropped |
| `POST_NOTIFICATIONS` runtime permission (NotificationExtensions) | none needed (portal) | |
| `NotificationCompat.Builder.setColor(accent_blue)` | dropped | |
| `WorkManager.getWorkInfosByTag` (isRunning) | `JobRegistry` | |
| `CoroutineWorker.setForeground`, `delay(500)` (setForegroundSafely) | dropped | |
| `Settings.Global.ANIMATOR_DURATION_SCALE` (animatorDurationScale) | `gtk::Settings::gtk_enable_animations` + KDE `AnimationDurationFactor` | |
| `UiModeManager`, `Configuration.uiMode` night mask (isNightMode, createReaderThemeContext) | `ashpd` Settings `color-scheme` | |
| `Configuration.smallestScreenWidthDp`, `orientation` (DisplayExtensions) | window width/height in px | |
| `Window.FLAG_SECURE` (setSecureScreen) | dropped | |
| `DisplayCutout`, `rootWindowInsets` (hasDisplayCutout) | dropped | |
| `config_navBarNeedsScrim` via `Resources.getSystem().getIdentifier` (InternalResourceHelper) | dropped | |
| `Resources.getSystem().displayMetrics` (isVisibleOnScreen) | window bounds via `compute_bounds` | |
| `View.getGlobalVisibleRect`, `Rect.intersect` (isVisibleOnScreen) | `gtk::Widget::compute_bounds` + `geometry::intersects_positive_area` | |
| `PopupMenu` (popupMenu) | `gtk::PopoverMenu` with `gio::Menu` | |
| `ComposeView`, `setContent`, `TachiyomiTheme` (setComposeContent) | dropped (GTK widgets) | |
| `BiometricManager`, `BiometricPrompt`, `startClass2BiometricOrCredentialAuthentication` (AuthenticatorUtil) | PAM (`pam` crate) + optional fprintd | section 2.18 |
| `PackageManager`, `getApplicationInfo` (isPackageInstalled) | dropped | |
| `Build.MANUFACTURER`, `Build.BRAND`, `Build.DEVICE`, `Build.MODEL` (CrashLogUtil) | `/sys/class/dmi/id/*` | |
| `Build.VERSION.RELEASE`, `SDK_INT`, `DISPLAY` | `/etc/os-release` `PRETTY_NAME`, `/proc/sys/kernel/osrelease` | |
| `WebViewUtil.getVersion` (CrashLogUtil) | GTK / GLib versions from `gtk::major_version()` | |
| `Runtime.exec("logcat ...")` (CrashLogUtil) | in-process ring buffer + `journalctl --user` hint | |
| `ActivityManager.isLowRamDevice` (not in scope; used by App) | `/proc/meminfo` MemTotal < 3 GiB | |
| `ClassLoader`, `PathClassLoader`, `DexFile` (ChildFirstPathClassLoader) | dropped (extension design in area 20) | |
| `TvUtils` `UiModeManager.TYPE_TELEVISION`, `Intent.ACTION_OPEN_DOCUMENT` checks | dropped | |
| `DynamicColors.isDynamicColorAvailable` (DeviceUtilExtensions) | dropped (GTK theme) | |
| `DeviceUtil.isSamsung`, `isMiui` (system property `ro.miui.ui.version.name`) | dropped | |
| `PowerManager` wake lock (powerManager) | `gtk::Application::inhibit(window, IDLE, reason)` | |
| `Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES` (launchRequestPackageInstallsPermission) | dropped | |
| `UniFile` / SAF (`UniFile.size`, `getUriSize`, `UniFile.fromUri`) | `std::fs` + `symlink_metadata` | |
| `Okio BufferedSource` (saveTo) | `tokio::io::AsyncRead` + `tokio::fs::File` | section 2.13 |
| `java.io.File.outputStream` / `mkdirs` | `std::fs::create_dir_all` | |
| `java.time.*` (DateExtensions, AniChartApi) | `chrono` (+ `chrono-tz` for explicit zones) | |
| `java.text.DateFormat`, `DateTimeFormatter.ofLocalized*` | `icu_datetime` (Length::Short) | |
| `java.util.Locale`, `LocaleListCompat` (LocaleHelper) | ICU4X `icu_displaynames`, `glib::language_names()` | |
| `java.text.Collator` (SortUtil compareToWithCollator) | `icu_collator` Primary | |
| plural resources (`pluralStringResource`) | `icu_plurals` + key table | |
| `SecureRandom` + `Base64.getUrlEncoder().withoutPadding()` (PkceUtil) | `rand::rngs::OsRng` + `base64` URL_SAFE_NO_PAD | |
| `OkHttpClient`, `Request`, `POST`/`GET` helpers (AniChartApi) | `reqwest::Client` (area 12 shared client) | 10 s timeouts |
| `kotlinx.serialization` `buildJsonObject` (AniChartApi) | `serde_json::json!` | |
| `OffsetDateTime.parse` (toUnixTimestamp) | `chrono::DateTime::parse_from_rfc3339` | |
| `Calendar.getInstance().timeInMillis` | `chrono::Utc::now().timestamp_millis()` | |
| `Hash.md5` (DiskUtil) | `md5` crate | must stay MD5 for cache-name parity |
| RxJava `Observable.asFlow` (RxJavaExtensions) | dropped | |
| `callbackFlow`, `Flow`, `StateFlow` (NetworkStateTracker, PreferenceMutableState) | `tokio::sync::watch` | |
| `Injekt.get<T>()` (many) | explicit constructor injection (area 01) | |
| `Preference<T>.changes()` (PreferenceMutableState) | `kuro_prefs::Pref<T>::subscribe()` | |
| `Modifier`/Compose `remember` + `collectAsState` (AnimeSourceUtil, MangaSourceUtil) | `watch::Receiver` + `glib::spawn_future_local` | |
| `Context.toast(...)` (many) | `ui::toast::Toaster` | section 5 |
| `Activity.window.setSecureScreen` (SecureActivityDelegate) | dropped, note in settings | |
| `Drawable`, `BitmapDrawable`, Coil `ScaleDrawable` (DrawableExtensions) | `gdk::Texture` | |

---

## 7. Pitfalls

1. `gio::NetworkMonitor` reports connectivity `FULL` only when the portal check succeeds. On networks behind captive portals the state is `PORTAL`, which the Rust port treats as offline (matches upstream `isValidated` = false).
2. NetworkManager is absent on some systems (for example, `systemd-networkd` only). `watch_network` must fall back to `gio::NetworkMonitor` alone and set `is_wifi = false`.
3. `gdk::Clipboard::set_text` silently does nothing if the window is not focused on some Wayland compositors; show the toast only when the call returns without a clipboard error, and accept that the toast can be shown even when paste fails. Document it.
4. `ashpd` portal calls fail when there is no session bus (`DBUS_SESSION_BUS_ADDRESS` unset). Every call must have a fallback (`NoPreference` for colour scheme).
5. `glib::user_cache_dir()` returns the XDG cache dir; under Flatpak the path is sandboxed. The design does not target Flatpak initially.
6. PAM needs the service file `/etc/pam.d/kurobayomi` installed by the package. Without it, `is_authentication_supported()` must return `false` and the app lock UI must explain why (no silent "allowed").
7. ICU4X data: enable `compiled_data` (default) to avoid runtime data files; binary size grows by a few MB.
8. `chrono::NaiveDateTime` arithmetic is not DST aware, which matches `LocalDateTime` in upstream.
9. `Double` chapter numbers: use `f64::to_bits` after normalising `-0.0` for hash keys (behaviour 5.7).
10. The Simkl calendar JSON is large; read with a size cap and parse with `serde_json::from_reader` on a blocking thread, not on the GTK thread.
11. `gtk::UriLauncher` is available since GTK 4.10; with `v4_16` enabled it is always present.
12. `gtk::FileDialog` (GTK 4.10+) is async: wrap the call in `glib::spawn_future_local` with `save_future`.
13. `dir_size` on very large download trees is slow; run on `spawn_blocking` and cache the result for the storage screen for 30 s (Proposal).
14. `Notifier` posts need an `Application` with an id; the mockup's `gtk::Application` must have an application id set, or the portal rejects the notification.
15. Keep upstream labels untranslated only where upstream does (`B`, `KB`, `MB`, `GB`, debug labels). All user-visible strings use `kuro-i18n`.
