# Area 47 (part 1) — Rust design

Target: crate `kurobayomi` (edition 2024), gtk4-rs 0.11 with feature `v4_16`, no libadwaita. Current `Cargo.toml` has `gtk`, `serde`, `serde_json`. This design adds the crates listed in section 7.

The Kotlin settings tree is data-driven (`Preference` objects built in Composables). The mockup's `spec/settings.json` is a design catalogue only (see `spec/SCHEMA.md`); the real app must not render from JSON, because the values have to come from the store. The Rust build therefore builds the tree in code, with typed handles, and keeps the JSON only as a documentation/test fixture.

---

## 1. Crate and module layout

```
src/
  main.rs                       (existing; add `mod settings` path change only)
  settings/
    mod.rs                      dispatch: body(id) -> Option<Built>   (replaces src/settings.rs)
    model.rs                    Node, Row, RowKind, ScreenDef (section 4)
    store.rs                    PrefStore (SQLite kv), Pref<T>, change channels (section 2)
    prefs.rs                    `Prefs` struct: every key of data doc section 2, typed (section 3)
    enums.rs                    ReadingMode, ReaderOrientation, TappingInvertMode, ReaderHideThreshold,
                                FlashColor, AutoTrackState, SecureScreenMode, DataSaver, ExtensionInstaller,
                                EpisodeSwipeAction, ChapterSwipeAction, StartScreen, NavStyle, TabletUiMode,
                                ThemeMode, AppTheme (serde names = Android constant names)
    labels.rs                   category_label(), include/exclude text (T-47a-005)
    search.rs                   SearchIndex::build(&Prefs) -> Vec<Hit>; query() (T-47a-004)
    widgets.rs                  row builders: switch_row, list_row, multi_row, slider_row, text_row,
                                edit_row, info_row, action_row, link_row, tracker_row, group_header
    dialogs.rs                  modal builders: radio_dialog, multi_dialog, text_dialog, confirm_dialog,
                                tri_state_dialog, numeric_chooser_dialog
    auth.rs                     confirm(title, subtitle) -> Result<bool> (T-47a-068)
    screens/
      mod.rs                    register all screens by id (used by ui::open)
      appearance.rs             T-47a-010..019
      library.rs                T-47a-020..030
      reader.rs                 T-47a-031..043
      downloads.rs              T-47a-044..052
      tracking.rs               T-47a-053..059
      browse.rs                 T-47a-060..062
      security.rs               T-47a-063..068
      advanced.rs               T-47a-069..087
      data.rs                   T-47a-088..092
      about.rs                  T-47a-093..095
      licenses.rs               T-47a-096..097
      clear_db.rs               T-47a-075 (manga + anime variants)
      main.rs                   T-47a-003, T-47a-098 (menu; sidebar)
  settings_backend/             (not in this area; referenced) tracker creds (T-47a-059), auth (T-47a-063)
```

Rule: a file in `settings/screens/` exports one `pub fn build(ctx: &SettingsCtx) -> Vec<Node>` plus any local dialogs. No file in this area exceeds ~500 lines; `reader.rs` is the largest.

---

## 2. Preference store

### 2.1 Storage

SQLite table (in the app database opened by area 03):

```sql
CREATE TABLE IF NOT EXISTS prefs (
    key   TEXT PRIMARY KEY NOT NULL,   -- Android key name, including __APP_STATE_/__PRIVATE_ prefix
    value TEXT NOT NULL                -- JSON-encoded value
) WITHOUT ROWID;
```

Value encoding (JSON, `serde_json`):
- Bool → `true`/`false`; Int → number; Long → number (fits i64); String → string; StringSet → array of strings sorted ascending on write; Enum → string of the Rust variant's serde name, which equals the Android constant name (for example `"ToggleRead"`, `"RIGHT_TO_LEFT"` only where the Android value was an enum name; ReadingMode and ReaderOrientation are stored as ints, see below).

Int-coded enums (Android stores the integer): `pref_default_reading_mode_key` (int flagValue), `pref_default_orientation_type_key` (int flagValue), `pref_reader_theme_key` (int), `pref_image_scale_type_key` (1-based int), `pref_zoom_start_key` (1-based int), `reader_navigation_mode_pager`/`_webtoon` (int), `pref_double_tap_anim_speed` (int ms), `doh_provider` (int), `data_saver_image_quality` (int), `remove_after_read_slots` (int), `download_slots` (int), `backup_interval` (int hours), `pref_library_update_interval_key` (int hours), `lock_app_after` (int minutes, -1 = never).

Enum stored by name (Android `getEnum`): `pref_theme_mode_key`, `pref_app_theme`, `tablet_ui_mode`, `start_screen`, `bottom_rail_nav_style`, `reader_hide_threshold`, `pref_reader_flash_mode`, `reader_tapping_inverted`, `reader_tapping_inverted_webtoon`, `secure_screen_v2`, `extension_installer`, `data_saver`, `pref_auto_update_manga_on_mark_read`, `pref_migration_sorting`, `pref_migration_direction`, `pref_episode_swipe_*_action`, `pref_chapter_swipe_*_action`.

Rust reads use the Android names; the Rust enum `#[serde(rename = "…")]` maps the Android constant. The Android legacy default for an unknown enum name is the default (matches `enumValueOf` catch in `PreferenceStore.kt`).

### 2.2 Types

```rust
// src/settings/store.rs
use serde::{de::DeserializeOwned, Serialize};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

/// In-memory cache of every row, loaded once at startup. GTK-thread reads hit this only.
#[derive(Clone)]
pub struct PrefStore {
    cache: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    tx: tokio::sync::mpsc::UnboundedSender<WriteOp>,     // drained by the writer task (section 5)
    notify: Arc<RwLock<HashMap<String, Vec<async_channel::Sender<()>>>>>,
}

pub enum WriteOp {
    Set { key: String, value: String },
    Delete { key: String },
}

impl PrefStore {
    pub async fn open(conn: tokio_rusqlite::Connection) -> Result<Self, StoreError>;
    pub fn raw(&self, key: &str) -> Option<serde_json::Value>;
    pub fn set_raw(&self, key: &str, v: serde_json::Value);   // updates cache, enqueues WriteOp, notifies
    pub fn delete(&self, key: &str);
    pub fn subscribe(&self, key: &str) -> async_channel::Receiver<()>;
}

/// Typed handle. Clone is cheap.
#[derive(Clone)]
pub struct Pref<T> {
    store: PrefStore,
    key: &'static str,
    default: fn() -> T,
    _t: std::marker::PhantomData<fn() -> T>,
}

impl<T: Serialize + DeserializeOwned + Clone> Pref<T> {
    pub fn get(&self) -> T;                      // cache lookup; on parse failure: default + tracing::warn
    pub fn set(&self, value: T);                 // serialize + set_raw
    pub fn reset(&self);                         // delete key (Android delete())
    pub fn is_set(&self) -> bool;
    pub fn key(&self) -> &'static str;
    pub fn changes(&self) -> async_channel::Receiver<()>; // fires after set/reset
}
```

Rules (from the Android behaviour in data doc section 1.1):
- `get()` never fails. A value that does not deserialize returns the default and logs `warn` once per key per process.
- `set()` updates the cache synchronously, so the next `get()` on the GTK thread sees it immediately.
- `reset()` removes the cache entry and enqueues a delete (Android `delete()` semantics).
- `changes()` notifies after `set` or `reset` on the GTK thread; subscribers re-read with `get()`.

### 2.3 Sets

String sets are `BTreeSet<String>` in Rust (sorted) with JSON arrays. Ordering of the Android `HashSet` is not observable in the UI, so sorted order is safe.

### 2.4 Object-valued prefs

`LibraryDisplayMode`, `MangaLibrarySort`, `AnimeLibrarySort` use the Android custom serializers (`serialize`/`deserialize`). In Rust they implement `Serialize`/`Deserialize` as strings with the same literal values (`"COMPACT_GRID"`, `"COMFORTABLE_GRID"`, `"LIST"`, `"COVER_ONLY_GRID"`), so `Pref<LibraryDisplayMode>` works without a custom serializer. The sort-mode string formats are owned by area 08.

---

## 3. Typed `Prefs` struct

One struct owns every preference used by the scope, so screens never spell keys.

```rust
// src/settings/prefs.rs
pub struct Prefs {
    pub theme_mode: Pref<ThemeMode>,                    // "pref_theme_mode_key", default SYSTEM
    pub app_theme: Pref<AppTheme>,                // "pref_app_theme", default DEFAULT (dropped, T-47a-012)
    pub theme_dark_amoled: Pref<bool>,                  // "pref_theme_dark_amoled_key", false
    pub relative_time: Pref<bool>,                      // "relative_time_v2", true
    pub date_format: Pref<String>,                      // "app_date_format", ""
    pub start_screen: Pref<StartScreen>,                // "start_screen", ANIME
    pub nav_style: Pref<NavStyle>,                      // "bottom_rail_nav_style", MOVE_HISTORY_TO_MORE

    pub library_update_interval: Pref<i32>,             // "pref_library_update_interval_key", 0
    pub library_update_restriction: Pref<BTreeSet<String>>, // "library_update_restriction", {"wifi"}
    pub library_update_manga_restriction: Pref<BTreeSet<String>>, // "library_update_manga_restriction", {4 values}
    pub default_manga_category: Pref<i32>,              // "default_category", -1
    pub default_anime_category: Pref<i32>,              // "default_anime_category", -1
    pub categorized_display: Pref<bool>,                // "categorized_display", false
    pub hidden_categories: Pref<bool>,                  // "hidden_categories", false
    pub auto_update_metadata: Pref<bool>,               // "auto_update_metadata", false
    pub show_updates_count: Pref<bool>,                 // "library_show_updates_count", true
    pub swipe_chapter_left: Pref<ChapterSwipeAction>,         // key "pref_chapter_swipe_end_action", ToggleRead
    pub swipe_chapter_right: Pref<ChapterSwipeAction>,        // key "pref_chapter_swipe_start_action", ToggleBookmark
    pub swipe_episode_left: Pref<EpisodeSwipeAction>,         // key "pref_episode_swipe_end_action", ToggleSeen
    pub swipe_episode_right: Pref<EpisodeSwipeAction>,        // key "pref_episode_swipe_start_action", ToggleBookmark
    // … one line per key in p1-data-and-config.md section 2 (233 rows)
}
```

Note on the swipe fields: the Rust field names are by UI position; the Android key names are kept (behaviour §3.6).

Construction:

```rust
impl Prefs {
    pub fn new(store: PrefStore) -> Self {
        Self {
            theme_mode: store.pref("pref_theme_mode_key", || ThemeMode::System),
            // …
        }
    }
}
```

`PrefStore::pref` is a generic constructor:

```rust
impl PrefStore {
    pub fn pref<T>(&self, key: &'static str, default: fn() -> T) -> Pref<T>
    where T: Serialize + DeserializeOwned + Clone { /* … */ }
}
```

The `Prefs` value is created once in `main()` and cloned into the UI context (`SettingsCtx`, section 4).

Shared context:

```rust
pub struct SettingsCtx {
    pub prefs: std::rc::Rc<Prefs>,            // GTK-thread only
    pub bg: BackgroundHandle,                 // tokio side: jobs, cache size, export (section 5)
    pub window: gtk::ApplicationWindow,
}
```

---

## 4. Settings tree model

```rust
// src/settings/model.rs
pub enum Node {
    Group { title: Option<String>, enabled: bool, rows: Vec<Row> },
    Row(Row),                                     // top-level rows, no group header
}

pub struct Row {
    pub title: String,
    pub subtitle: Option<String>,                 // already formatted (see section 6)
    pub enabled: bool,                            // false = hidden with a 300 ms reveal (T-47a-002)
    pub search_text: bool,                        // false for info/tracker rows (search exclusion)
    pub kind: RowKind,
}

pub enum RowKind {
    Switch { pref: Pref<bool>, on_change: Option<Box<dyn Fn(bool) -> bool>> },
    List   { pref: ListPref, options: Vec<(String, String)>, on_change: Option<Box<dyn Fn(&str) -> bool>> },
    Multi  { pref: Pref<BTreeSet<String>>, options: Vec<(String, String)>, on_change: Option<Box<dyn Fn(&BTreeSet<String>) -> bool>> },
    Slider { pref: Pref<i32>, range: std::ops::RangeInclusive<i32>, unit_label: Option<String> },
    Text   { pref: Pref<String>, can_be_blank: bool, validate: Option<fn(&str) -> bool> },
    Info,
    Action { on_click: Box<dyn Fn()> },
    Link   { target: ScreenId },
    Tracker { id: TrackerId, login: Box<dyn Fn()>, logout: Box<dyn Fn()> },
    Custom { build: Box<dyn Fn() -> gtk::Widget> },
}

/// ListPref erases the value type to a string key; the list stores option keys as strings.
pub struct ListPref { pub key: &'static str, pub get: Box<dyn Fn() -> String>, pub set: Box<dyn Fn(&str)> }

pub type ScreenId = &'static str;   // "settings_library", … (same ids as spec/settings.json)
```

Rules encoded here:
- `on_change` returning `false` keeps the stored value (Android `onValueChanged` contract, data doc §1.2).
- `enabled = false` on a `Row` or `Group` hides it (T-47a-002).
- `search_text` implements the Info/Tracker exclusions (behaviour §15).

Screen registration:

```rust
pub struct ScreenDef {
    pub id: ScreenId,
    pub title: String,
    pub build: fn(&SettingsCtx) -> Vec<Node>,
    pub header_action: Option<fn() -> gtk::Button>,   // tracking guide, storage FAQ
}
pub fn screens() -> &'static [ScreenDef];            // screens/mod.rs
```

Dynamic rows (counts, enabled state) are recomputed by re-running `build` on `changes()` signals of the prefs the row depends on. Example: the Downloads screen re-builds when `download_new_episode` changes.

---

## 5. Threading and async model

| Work | Thread / runtime | Mechanism |
|---|---|---|
| All widgets, dialogs, `Pref::get/set` | GTK main thread | `glib::MainContext` |
| Pref persistence (SQLite writes) | tokio task `prefs_writer` on the multi-thread runtime | `tokio::sync::mpsc::UnboundedSender<WriteOp>`; the task batches writes (`BEGIN … COMMIT` per 50 ms window) |
| Library/update jobs (area 26) | tokio tasks | `BackgroundHandle::spawn(job)` |
| Cache size (`ChapterCache.readableSize`), cache clear, library CSV export, storage stat | `tokio::task::spawn_blocking` | result sent back with `async_channel`, consumed by `glib::spawn_future_local` |
| Update check (about screen) | tokio + reqwest (area 35) | same channel pattern |
| Tracker login (`tracker.login`) | tokio | result to GTK via channel; button label switches to "Loading…" |
| Clear-DB list and delete | tokio + `spawn_blocking` for SQL | `ClearDbModel` in `clear_db.rs` holds `Rc<RefCell<State>>` on the GTK side |

Rules:
- Never call a blocking filesystem or DB function on the GTK thread. The cache-size row shows "Calculating…" until the blocking task returns (Proposal; Android computes it on composition, see edge cases).
- `Pref::get` must never block: it reads the `RwLock` cache only.
- The GTK-side subscription helper:

```rust
pub fn on_pref_change<F: Fn() + 'static>(rx: async_channel::Receiver<()>, f: F) {
    glib::spawn_future_local(async move {
        while rx.recv().await.is_ok() { f(); }
    });
}
```

- Long-running dialogs (login) hold a `glib::WeakRef` to their window; if the window is closed while a request is in flight, the result is dropped without touching widgets.

---

## 6. Background scheduling (decision required)

Android WorkManager gives constraints (network type, metered, charging, battery-not-low) and survives app death. On Linux there is no equivalent in-process guarantee.

Options:
- **A (recommended, Proposal):** in-process scheduler in tokio. Next-run timestamps are stored in SQLite (area 26). Work runs while the app is open. Optional autostart: the app writes `$XDG_CONFIG_HOME/autostart/io.github.anas1412.Kurobayomi.desktop` when the user enables "Run in background" (a new row not in the Android screen, to be added with explicit sign-off).
- **B:** systemd `--user` timer `kurobayomi-update.timer` (`OnUnitActiveSec=` = interval) that runs `kurobayomi --update-library`. Works without the GUI; more moving parts.

Constraint mapping (Proposal for both options):

| Android constraint (data doc §1) | Linux check | Source |
|---|---|---|
| `NetworkType.UNMETERED` / `NET_CAPABILITY_NOT_METERED` | `gio::NetworkMonitor::network_metered()` must be false | gio |
| `TRANSPORT_WIFI` (`DEVICE_ONLY_ON_WIFI`) | NetworkManager `ActiveConnection.Type == "802-11-wireless"` via `zbus` (`org.freedesktop.NetworkManager`) | zbus |
| `setRequiresCharging(true)` (`ac`) | UPower `OnBattery == false` via `zbus` (`org.freedesktop.UPower`) | zbus |
| `setRequiresBatteryNotLow(true)` (always) | UPower `Percentage >= 15` (Proposal threshold; Android's threshold is OS-defined) | zbus |
| `BackoffPolicy.LINEAR/EXPONENTIAL, 10 min` | in-process backoff `10 min × attempt` (linear) or `10 min × 2^attempt` (exponential) | tokio |
| Flex 10 min on periodic | next run = last run + interval; jitter ≤ 10 min | tokio |

Decision needed from the user: A or B, and whether autostart is wanted.

---

## 7. Crates (and why)

| Crate | Version (Proposal) | Used for | Why this one |
|---|---|---|---|
| `gtk4` (existing) | 0.11.5, feature `v4_16` | widgets, `FileDialog`, `UriLauncher`, `Application::inhibit`, `GLContext` | Already in use; `v4_16` gives `GLContext` and `Inhibit` APIs; `FileDialog` needs ≥4.10 |
| `glib` / `gio` (re-exported by gtk4) | with gtk | `MainContext`, `spawn_future_local`, `NetworkMonitor`, `AppInfo::launch_default_for_uri` | No extra dependency |
| `tokio` | 1 (`rt-multi-thread`, `sync`, `time`, `macros`) | writer task, jobs, `spawn_blocking` | Stated in the project brief |
| `tokio-rusqlite` | 0.6 | async wrapper over the single writer connection | Keeps `rusqlite::Connection` off the GTK thread |
| `rusqlite` | 0.32 (`bundled`) | DB | Stated in the project brief |
| `serde`, `serde_json` | existing | pref values, enum names | Existing |
| `async-channel` | 2 | change notifications GTK-side, job results | `Send` + works with `spawn_future_local` (unlike `tokio::sync::watch` on GTK) |
| `thiserror` | 2 | `StoreError`, `SettingsError` | Typed errors without boilerplate |
| `tracing`, `tracing-subscriber` (`env-filter`, `reload`) | 0.1 / 0.3 | logs; `verbose_logging` changes the filter live | `reload` handle implements T-47a-070 |
| `zbus` | 5 | UPower, NetworkManager, fprintd (decision §9) | Pure-Rust D-Bus client; no libdbus |
| `ashpd` | 0.11 | XDG Settings portal (`org.freedesktop.appearance` `color-scheme`, `accent-color`) | Maintained portal bindings; no GTK dependency |
| `keyring` | 3 (`sync-secret-service`, `crypto-rust`) | tracker secrets (T-47a-059) | Secret Service/KWallet on KDE; falls back per T-47a-059 |
| `argon2` | 0.5 | passphrase hash for app lock (decision §9) | RustCrypto, PHC strings |
| `chrono` | 0.4 | date formatting (T-47a-016) | `strftime` patterns |
| `reqwest` (existing plan, area 12) | 0.12 (`rustls-tls`, `hickory-dns`) | update check (area 35), DoH via `hickory-resolver` | Stated in the brief |
| `hickory-resolver` | 0.24 (`https-rustls`) | DoH providers (T-47a-077) | Pure Rust DoH with custom servers |
| `directories` | 5 | XDG paths | Replaces Android `Context.cacheDir`/`filesDir` |
| `cargo-about` (build tool, not a dependency) | latest | licence list (T-47a-096) | Generates the JSON embedded with `include_str!` |

Not used: libadwaita (brief), `gettext` (strings stay as `&'static str` constants in `settings/strings.rs`; i18n is a later decision), any Android-specific crate.

---

## 8. Android → Linux API mapping (scope-level)

| Android API (file) | Linux / Rust replacement | Task |
|---|---|---|
| `SharedPreferences` / `PreferenceStore` (`AndroidPreferenceStore`) | SQLite `prefs` table + `PrefStore` (§2) | T-47a-001 |
| Compose `AlertDialog` (dialogs in every screen) | `gtk::Window` modal (`transient_for`, `modal`), built in `settings/dialogs.rs` | T-47a-002 |
| `RadioButton` list dialog (`ListPreferenceWidget`) | `gtk::CheckButton` group inside a modal; Cancel only | T-47a-002 |
| `Switch` (`SwitchPreferenceWidget`) | `gtk::Switch`, row activation toggles | T-47a-002 |
| Compose `Slider`/`BaseSliderItem` | `gtk::Scale` (horizontal, `draw-value`, `digits = 0`) | T-47a-035 |
| `OutlinedNumericChooser` | `gtk::SpinButton` with `-`/`+` buttons, step 100, lower 0, no upper | T-47a-045 |
| `ActivityCompat.recreate` (pure black) | live CSS swap (`gtk::CssProvider`) | T-47a-014 |
| `AppCompatDelegate.setDefaultNightMode` | `gtk::Settings::set_gtk_application_prefer_dark_theme` + portal `color-scheme` | T-47a-011 |
| Monet dynamic colours (`DynamicColors`) | portal `accent-color` via `ashpd` | T-47a-013 |
| `Intent(ACTION_VIEW)` / `openInBrowser` / `LocalUriHandler` | `gtk::UriLauncher::new(uri).launch(Some(&window), None::<&gio::Cancellable>, |_| {})` (GTK 4.10+) | T-47a-056, T-47a-095 |
| `ClipboardManager` (`copyToClipboard`) | `gdk::Display::clipboard().set_text(&text)` | T-47a-093 |
| SAF `OpenDocumentTree` / `UniFile` | `gtk::FileDialog::select_folder` + `std::fs::metadata` | T-47a-088 |
| SAF `CreateDocument("text/csv")` | `gtk::FileDialog::save` with filter `*.csv` | T-47a-091 |
| SAF `GetContent("*/*")` (backup file) | `gtk::FileDialog::open` | T-47a-092 |
| `Toast` | in-window overlay label (`gtk::Overlay` + `gtk::Revealer`, 3 s) | all |
| `BiometricPrompt` / `BiometricManager` | passphrase dialog (argon2) and/or fprintd via `zbus` (§9) | T-47a-063, T-47a-067, T-47a-068 |
| `Settings.ACTION_APP_NOTIFICATION_SETTINGS` | `systemsettings kcm_notifications` (KDE) via `Command`, else hidden | T-47a-072 |
| `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` | not applicable; dropped | T-47a-073 |
| `WebView` data clearing | `webkit6::WebsiteDataManager::clear` if WebKitGTK is used (area 36); else no-op | T-47a-076 |
| `CookieJar.removeAll()` | `reqwest::cookie::Jar` replacement with empty `Jar` | T-47a-076 |
| `Runtime.exec("logcat …")` (crash logs) | `tracing` ring buffer (`VecDeque<String>`, Proposal capacity 5000) | T-47a-069 |
| `FLAG_SECURE` (secure screen) | not available on Wayland/X11; dropped | T-47a-066 |
| `Window.keepScreenOn` (keep screen on) | `gtk::Application::inhibit(Some(&win), InhibitFlags::IDLE, Some("Reading"))`, cookie released on close | T-47a-034 |
| Fullscreen (`WindowInsets`) | `gtk::Window::fullscreen()` / `unfullscreen()` | T-47a-034 |
| Display cutout | dropped | T-47a-043 |
| `ActivityInfo.SCREEN_ORIENTATION_*` (rotation) | not applicable on desktop; the `ReaderOrientation` value becomes a per-viewer layout hint (area 44) | T-47a-034 |
| Volume keys (`onKeyDown`) | `gtk::EventControllerKey` on the reader page for `XF86AudioRaiseVolume`/`XF86AudioLowerVolume` and PageUp/PageDown | T-47a-039 |
| `Build.VERSION.SDK_INT` checks | removed (desktop) | all |
| `DeviceUtil.isMiui`, MIUI installer | removed | T-47a-085, T-47a-076 |
| `Locale.getDefault()` / `DateTimeFormatter` | `chrono::Local` + `glib::DateTime::format("%x")` for the localized short date | T-47a-016 |
| `NumberFormat.getPercentInstance()` | format `value/100` as `"{}%"` with the locale decimal separator from `glib` (Proposal) | T-47a-038 |
| `Injekt.get<T>()` | explicit `Rc<Prefs>` / `BackgroundHandle` in `SettingsCtx` | all |
| Voyager `Navigator.push/replace` | `ui::push` / `ui::replace` (add `replace` to `ui.rs`) | T-47a-004 |
| `kotlinx.collections.immutable` | `Vec`, `BTreeMap` | all |
| moko-resources `stringResource` | `&'static str` constants (appendix in behaviour §18) | all |

---

## 9. App lock decision (Proposal, needs sign-off)

Android uses `BiometricManager` with `BIOMETRIC_WEAK | DEVICE_CREDENTIAL` (`AuthenticatorUtil.kt:87-90`). No Linux API has that exact meaning. Options:

| Option | Mechanism | Pros | Cons |
|---|---|---|---|
| A (recommended) | Local passphrase, argon2id (`argon2` crate), PHC string in keyring entry `kurobayomi-applock` | Works everywhere; no system service | Typing a passphrase on every unlock |
| B | fprintd over D-Bus (`net.reactivated.Fprint`: `Manager.GetDefaultDevice`, `Device.Claim`, `Device.VerifyStart`, `VerifyStatus` signal) via `zbus` | Matches "biometric" feel | Only with a fingerprint reader; needs fprintd installed |
| C | PAM (`pam` crate) with the user's password | Uses the login password | Needs a PAM service file; more security surface |

`confirm(title, subtitle)` (T-47a-068) shows one modal regardless of option. The switch (`use_biometric_lock`) is enabled when the chosen method is available (`auth_supported()` returns `Some(method)`).

---

## 10. Error handling

- `StoreError` (thiserror): `Sqlite(rusqlite::Error)`, `Serde(serde_json::Error)`, `Closed`. Surfaced only to the writer task; the UI never sees it. A failed write is logged at `error` and retried once after 250 ms; a second failure shows a toast "Could not save setting" (Proposal text) and keeps the cache value.
- `SettingsError` (thiserror) for screen actions: `Io(std::io::Error)`, `Dbus(zbus::Error)`, `Keyring(keyring::Error)`, `Network(reqwest::Error)`. Each maps to a user toast with the Android text where one exists (behaviour §13) and to `tracing::error!` with the source chain.
- No `unwrap()` on user input, file paths, or D-Bus replies. A missing UPower or NetworkManager service is treated as "constraint unknown → allow" with a `warn` log.
- Parsing of stored JSON never panics; invalid values fall back to the default (section 2.2).

---

## 11. Integration with the existing mockup

| Mockup today | Change |
|---|---|
| `src/settings.rs` (264 lines, `body(id)` with hard-coded lists and `with_spec`) | becomes `src/settings/mod.rs` + `screens/*`. `body(id)` keeps its signature and dispatches to `screens::by_id(id)`. |
| `spec/settings.json` read via `spec.rs::with_spec` | kept as the UI catalogue for other areas; settings screens stop reading it. Items show the real store values. |
| `spec.rs::Item` enum (`Switch`, `List`, `Multi`, …) | retained for the other areas' specs; the settings tree uses `model::Row`. |
| `ui::item_row(&Item)` | replaced by `settings::widgets::row(&Row)`; `ui::row(title, subtitle, suffix)` remains and is used inside the widget builders. |
| `ui::show_dialog(&Dialog)` (spec-driven) | not used for settings; `settings::dialogs::*` builders are used instead. |
| `ui::open(id)`, `ui::push`, `ui::pop`, `ui::page`, `ui::scrolled`, `ui::column`, `ui::boxed_list` | reused as-is. Add `ui::replace(id)` for search jumps (`navigator.replace`). |
| `ui::resolve(id, label)` (button → dialog routing) | not used by settings; the settings screens own their dialogs. |
| `settings::search()` (search over spec) | replaced by `settings::search::SearchIndex` built from `Node` trees. |
| `settings::clear_db()` (static list) | replaced by `screens::clear_db` with live data (behaviour §14). |
| `settings::licenses()` (static text) | replaced by `screens::licenses` with the embedded JSON. |

Mockup visuals to keep: `boxed_list` rows with the trailing `go-next-symbolic` icon for link rows; the 56 px minimum row height; the `scrolled` page with 16 px margins.

---

## 12. Pitfalls

1. GTK widgets must be created and dropped on the GTK thread. `Pref<T>` is `Clone` but not `Send` in its GTK use; keep `Prefs` in an `Rc`.
2. `gtk::ListBox` row activation fires for the switch's own click too. Use `gtk::Switch::set_can_target(false)` and handle row activation only (matches Android's row toggle).
3. Re-entrancy: a `set` inside a `changes()` handler re-runs `build`, which re-creates rows and can recurse. Guard with a `Cell<bool>` "rebuilding" flag, and only rebuild rows whose visibility changed.
4. `gtk::Revealer` 300 ms slide for hidden rows: do not destroy the widget, or the search index and highlight would lose their target.
5. `gtk::FileDialog` returns `gio::File`; its path is `file.path()` only for local files. For `content://`-style or `portal` URIs the path may be `None`; treat `None` as "invalid location" and show `Invalid location: <uri>`.
6. Portal settings (`ashpd`) can be unavailable outside Flatpak/xdg-desktop-portal; fall back to `gtk::Settings` and hide "Follow desktop accent" when neither has a value.
7. `keyring` may block; call it only from `spawn_blocking`.
8. Dialog width: use `set_default_size` and `set_size_request(320, -1)` to avoid the 2-line title ellipsis issue (behaviour §1).
9. `inhibit()` returns a cookie that must be `uninhibit()`ed on reader close; leaking it keeps the screen awake across sessions.
10. The Mullvad DoH URL has a leading space in the source (data doc §4). The Rust URL constant must be trimmed; a test asserts it parses.

---

## 13. Tests for the design (summary)

Unit tests (cargo test): store round-trips for every `Pref` type; default fallback on bad JSON; enum name compatibility with the Android constants (table in data doc §1.3); category label function (edge cases file §3); search index filters (edge cases §5); the enable/disable rules (behaviour §4 table). Integration tests (GTK, `KUROBAYOMI_SMOKE` already used by the mockup): open each settings screen and assert the row count equals the number of visible rows in the behaviour spec.
