# 47-settings-screens-general — part 2 Rust design

Target: Kurobayomi, Rust stable, gtk4-rs 0.11 (GTK 4.16+, no libadwaita), tokio, rusqlite, reqwest, serde. Mockup code is in `/home/blackbox/kurobayomi/src/` (`settings.rs`, `screens.rs`, `ui.rs`, `spec.rs`, `main.rs`) and the spec in `/home/blackbox/kurobayomi/spec/settings.json`.

---

## 1. Crate dependencies (for this area only)

```toml
[dependencies]
gtk4 = "0.11"                 # GTK 4 widgets, gtk::FileDialog, gdk::Clipboard, gtk::AlertDialog
glib = "0.22"                 # glib::spawn_future_local, MainContext
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "fs", "sync"] }
async-channel = "2"           # IO task -> GTK main thread messages
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "gzip", "json"] }
rusqlite = { version = "0.32", features = ["bundled"] }   # extension_store, extension_repos (same schema as Aniyomi)
serde = { version = "1", features = ["derive"] }
serde_json = "1"
flate2 = "1"                  # gzip for index bodies and .tachibk (area:21/22)
prost = "0.13"                # protobuf for index bodies and Backup (area:21/22)
rustix = { version = "0.38", features = ["fs", "system"] }   # statvfs, uname
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
icu_displaynames = "2"        # native language names (ICU4X), replaces LocaleHelper display names
url = "2"                     # URL normalisation, replaces OkHttp HttpUrl.toHttpUrlOrNull
thiserror = "2"
tracing = "0.1"

[build-dependencies]
# none. build.rs uses std::process::Command for git and pkg-config.
```

Why each one:
- `gtk4` / `glib`: the UI toolkit the mockup already uses.
- `async-channel`: `glib::MainContext::channel` is deprecated; `async-channel` + `spawn_future_local` is the supported replacement.
- `reqwest` with rustls: no OpenSSL dependency on CachyOS builds; gzip decoding handled by `flate2` for explicit control over the magic-byte check the Kotlin does.
- `rusqlite` bundled: avoids a system SQLite version mismatch; same table names as Aniyomi.
- `rustix`: `statvfs` without `libc` unsafe code.
- `icu_displaynames`: the Kotlin uses `java.util.Locale.getDisplayName(locale)`. ICU gives the same kind of output (native names). If the ICU data size is unwelcome, the lazy fallback is to show the tag. Recorded as a ceiling, not built now.
- `url`: needed to reproduce `toHttpUrlOrNull()` then `toString()` normalisation before the regex.

Not used: `libadwaita` (forbidden by the brief), `async-trait` (use concrete types), `anyhow` in library code (only at the binary edge).

---

## 2. Module layout

Existing mockup: `src/settings.rs` is the module root for settings (it already has `body(id)` and helpers). Rust 2018+ allows `src/settings.rs` plus `src/settings/*.rs`. Add:

```
src/settings.rs                 (existing; keeps body() dispatch and part-1 pages)
src/settings/shell.rs           routing, two-pane, back navigation        (T-47b-001..003)
src/settings/toast.rs           in-window toast overlay                    (T-47b-052)
src/settings/app_language.rs    language list, apply                       (T-47b-005..008)
src/settings/repos/mod.rs       shared repo model and UI helpers
src/settings/repos/model.rs     AnimeStore, MangaRepo, dialog/state enums  (see sections 3, 4)
src/settings/repos/anime.rs     anime store page + model                   (T-47b-009..018)
src/settings/repos/manga.rs     manga repo page + model                    (T-47b-019..026)
src/settings/repos/dialogs.rs   all dialogs (both kinds)                   (T-47b-016/017/021/024/025)
src/settings/repos/store.rs     rusqlite access (extension_store, extension_repos) (T-47b-009, T-47b-022)
src/settings/backup/mod.rs
src/settings/backup/options.rs  BackupOptions, RestoreOptions, OptionEntry  (T-47b-027, T-47b-028)
src/settings/backup/create.rs   create page + save dialog + job hand-off     (T-47b-029, T-47b-030)
src/settings/backup/restore.rs  restore page, validation state machine       (T-47b-031..034, T-47b-036)
src/settings/backup/validator.rs missing sources/trackers                    (T-47b-032)
src/settings/storage.rs         mount scan, statvfs, size format             (T-47b-037..039)
src/settings/debug/mod.rs       text-dump page helper                        (T-47b-047)
src/settings/debug/info.rs      app and device info                          (T-47b-040, 041, 048, 049, 050)
src/settings/debug/workers.rs   worker info page                             (T-47b-043)
src/settings/debug/schema.rs    backup schema page                           (T-47b-045)
src/jobs/registry.rs            job registry (area:26 uses it too)           (T-47b-044)
src/util/clipboard.rs           copy + toast, truncate_center(50)            (T-47b-046)
src/util/fmt_size.rs            SI size formatter                            (T-47b-039)
build.rs                        git SHA, build time, webkit version          (T-47b-049)
proto/backup.proto              protobuf definition (area:21), used by schema.rs
```

Mockup helper names used below (`built`, `Built`, `column`, `boxed_list`, `row`, `scrolled`, `status`, `toolbar`, `icon_button`, `margins`, `menu_button`, `with_spec`, `screen`, `open`, `show_dialog`) already exist in `src/screens.rs`, `src/ui.rs`, `src/spec.rs`, `src/settings.rs`.

---

## 3. Key types

### 3.1 Settings shell

```rust
// src/settings/shell.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SettingsDestination {
    About = 0,
    DataAndStorage = 1,
    Tracking = 2,
}

pub const TABLET_UI_MIN_WIDTH_DP: i32 = 720;

pub struct SettingsShell {
    root: gtk::Box,                          // outermost widget added to the window
    paned: Option<gtk::Paned>,               // Some(..) when two-pane is active
    stack: gtk::Stack,                       // detail pages
    history: std::cell::RefCell<Vec<&'static str>>, // page ids, bottom = root page
}

impl SettingsShell {
    /// `None` = default start page (Main on narrow, Appearance on wide).
    pub fn open(dest: Option<SettingsDestination>) -> SettingsShell;
    pub fn is_two_pane(&self) -> bool;          // width >= TABLET_UI_MIN_WIDTH_DP
    pub fn back(&self) -> BackOutcome;           // pop inner page or close shell
}

pub enum BackOutcome { PoppedInner, CloseShell }
```

Two-pane: `gtk::Paned` (start child = Main list, end child = `gtk::Stack` of pages). Narrow: one `gtk::Stack` with Main as root. Switch on `connect_notify_local(Some("default-width"))` and on `width` changes via `gtk::Widget::connect_map` plus size-allocate. Keep the decision a pure function `fn two_pane(width: i32) -> bool { width >= TABLET_UI_MIN_WIDTH_DP }` so it is unit-testable (see `p2-edge-cases-and-tests.md`).

### 3.2 App language

```rust
// src/settings/app_language.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Language {
    pub tag: String,                    // "" = system default
    pub display_name: String,           // English name
    pub localized_name: Option<String>, // native name, None for the default row
}

/// Ordered list: default row first, then by native (localised) name.
pub fn language_list(catalog_tags: &[&str], system_tag: &str) -> Vec<Language>;

pub fn apply_language(tag: &str) -> Result<(), LanguageError>; // writes settings key only; restart applies
```

Sorting rule (Kotlin `sortBy { displayName }` on the localised name): sort by the native name with `str::cmp` after lowercasing and collation-free comparison. Do not reproduce Java collation; document the difference.

### 3.3 Repo models and state

```rust
// src/settings/repos/model.rs
pub struct AnimeStore {
    pub index_url: String,
    pub name: String,
    pub badge_label: String,
    pub signing_key: String,
    pub contact_website: String,
    pub contact_discord: Option<String>,
    pub is_legacy: bool,
    pub extension_list_url: Option<String>,
}

pub struct MangaRepo {
    pub base_url: String,
    pub name: String,
    pub short_name: Option<String>,
    pub website: String,
    pub signing_key_fingerprint: String,
}

pub enum AnimeDialog {
    Create { processing: bool, error: Option<String> },
    Delete { store: AnimeStore },
    Confirm { url: String, already_exists: bool, processing: bool, error: Option<String> },
}

pub enum AnimeScreenState {
    Loading,
    Ready { stores: Vec<AnimeStore>, dialog: Option<AnimeDialog> },
}

pub enum MangaDialog {
    Create,
    Delete { base_url: String },
    Conflict { old: MangaRepo, new: MangaRepo },
    Confirm { url: String },
}

pub enum MangaScreenState {
    Loading,
    Ready { repos: Vec<MangaRepo>, dialog: Option<MangaDialog> },
}

pub enum RepoMessage {                 // was RepoEvent.LocalizedMessage
    InvalidUrl,                        // "Invalid repo URL"
    RepoAlreadyExists,                 // "This repo already exists!"
}
```

Enum variants carry data instead of Kotlin's nullable `dialog` plus separate flags, so illegal combinations are not representable. The Kotlin `oldRepos` field is dropped (dead).

### 3.4 Create outcome (from area:18 and area:19 services)

```rust
pub enum MangaCreateOutcome {
    Success,
    InvalidUrl,
    RepoAlreadyExists,
    DuplicateFingerprint { old: MangaRepo, new: MangaRepo },
    Error,
}

pub enum AnimeAddError {
    /// Carries the text shown in the dialog. Kotlin uses throwable.message ?: "unknown error".
    Message(String),
}
```

### 3.5 Backup options

```rust
// src/settings/backup/options.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupOptions {
    pub library_entries: bool,   // default true
    pub categories: bool,        // true
    pub chapters: bool,          // true
    pub tracking: bool,          // true
    pub history: bool,           // true
    pub read_entries: bool,      // true
    pub app_settings: bool,      // true
    pub extension_stores: bool,  // true
    pub custom_button: bool,     // true
    pub source_settings: bool,   // true
    pub private_settings: bool,  // false
    pub extensions: bool,        // false
}

impl Default for BackupOptions { /* defaults above */ }

impl BackupOptions {
    pub fn to_array(self) -> [bool; 12];          // order from p2-data-and-config.md section 10
    pub fn from_array(a: [bool; 12]) -> Self;
    pub fn can_create(self) -> bool {
        self.library_entries || self.categories || self.app_settings
            || self.extension_stores || self.custom_button || self.source_settings
    }
}

#[derive(Clone, Copy)]
pub struct OptionEntry<T: Copy> {
    pub label: &'static str,                     // message key resolved at build time
    pub get: fn(&T) -> bool,
    pub set: fn(&mut T, bool),
    pub enabled: fn(&T) -> bool,                 // always true for restore
}

fn always(_: &BackupOptions) -> bool { true }

pub const BACKUP_LIBRARY_OPTIONS: [OptionEntry<BackupOptions>; 6] = [
    OptionEntry { label: "Library entries", get: |o| o.library_entries, set: |o, v| o.library_entries = v, enabled: always },
    OptionEntry { label: "Chapters and episodes", get: |o| o.chapters, set: |o, v| o.chapters = v, enabled: |o| o.library_entries },
    OptionEntry { label: "Tracking", get: |o| o.tracking, set: |o, v| o.tracking = v, enabled: |o| o.library_entries },
    OptionEntry { label: "History", get: |o| o.history, set: |o, v| o.history = v, enabled: |o| o.library_entries },
    OptionEntry { label: "Categories", get: |o| o.categories, set: |o, v| o.categories = v, enabled: always },
    OptionEntry { label: "All read/watched entries", get: |o| o.read_entries, set: |o, v| o.read_entries = v, enabled: |o| o.library_entries },
];
pub const BACKUP_SETTINGS_OPTIONS: [OptionEntry<BackupOptions>; 5] = [
    OptionEntry { label: "App settings", get: |o| o.app_settings, set: |o, v| o.app_settings = v, enabled: always },
    OptionEntry { label: "Extension stores", get: |o| o.extension_stores, set: |o, v| o.extension_stores = v, enabled: always },
    OptionEntry { label: "Custom buttons", get: |o| o.custom_button, set: |o, v| o.custom_button = v, enabled: always },
    OptionEntry { label: "Source settings", get: |o| o.source_settings, set: |o, v| o.source_settings = v, enabled: always },
    OptionEntry { label: "Include sensitive settings (e.g., tracker login tokens)", get: |o| o.private_settings, set: |o, v| o.private_settings = v, enabled: |o| o.app_settings || o.source_settings },
];
pub const BACKUP_EXTENSION_OPTIONS: [OptionEntry<BackupOptions>; 1] = [
    OptionEntry { label: "Extensions", get: |o| o.extensions, set: |o, v| o.extensions = v, enabled: always },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreOptions {
    pub library_entries: bool,   // default true
    pub categories: bool,        // true
    pub app_settings: bool,      // true
    pub extension_stores: bool,  // true
    pub custom_buttons: bool,    // true
    pub source_settings: bool,   // true
    pub extensions: bool,        // false
}
impl RestoreOptions {
    pub fn can_restore(self) -> bool;             // any of 7
    pub fn to_array(self) -> [bool; 7];
}
pub const RESTORE_OPTIONS: [OptionEntry<RestoreOptions>; 7] = [
    OptionEntry { label: "Library", get: |o| o.library_entries, set: |o, v| o.library_entries = v, enabled: always_r },
    OptionEntry { label: "Categories", get: |o| o.categories, set: |o, v| o.categories = v, enabled: always_r },
    OptionEntry { label: "App settings", get: |o| o.app_settings, set: |o, v| o.app_settings = v, enabled: always_r },
    OptionEntry { label: "Extension stores", get: |o| o.extension_stores, set: |o, v| o.extension_stores = v, enabled: always_r },
    OptionEntry { label: "Custom buttons", get: |o| o.custom_buttons, set: |o, v| o.custom_buttons = v, enabled: always_r },
    OptionEntry { label: "Source settings", get: |o| o.source_settings, set: |o, v| o.source_settings = v, enabled: always_r },
    OptionEntry { label: "Extensions", get: |o| o.extensions, set: |o, v| o.extensions = v, enabled: always_r },
];
fn always_r(_: &RestoreOptions) -> bool { true }
```

`fn` pointers instead of closures keep the tables `const` and `Copy`. Labels are `&'static str` resolved from the message catalog (see section 7).

### 3.6 Restore validation

```rust
// src/settings/backup/validator.rs
pub struct ValidationResults {
    pub missing_sources: Vec<String>,   // manga then anime, each sorted, deduped
    pub missing_trackers: Vec<String>,  // sorted, deduped
}

pub enum ValidationError {
    /// Decode failed. `chain` = error and each cause, joined with ": " (proposal; Kotlin shows wrapper toString).
    Invalid { chain: String },
}

pub trait SourceCatalog {
    fn has_manga_source(&self, id: i64) -> bool;
    fn manga_source_name(&self, id: i64) -> String;   // getOrStub(id).toString()
    fn has_anime_source(&self, id: i64) -> bool;
    fn anime_source_name(&self, id: i64) -> String;
}
pub trait TrackerCatalog {
    fn tracker_name_if_not_logged_in(&self, sync_id: i64) -> Option<String>; // None when unknown or logged in
}

pub fn validate(
    backup: &DecodedBackup,              // area:22 decoder output
    sources: &dyn SourceCatalog,
    trackers: &dyn TrackerCatalog,
) -> ValidationResults;
```

The two catalog traits exist so the logic is testable without the extension system. This is the one place a trait is justified (ponytail allows it: two real implementations, the app and the test double).

### 3.7 Storage

```rust
// src/settings/storage.rs
pub struct StorageRoot { pub path: PathBuf, pub dev: u64 }
pub struct Usage { pub available: u64, pub total: u64 }

pub fn storage_roots(mountinfo: &str, data_dir: &Path) -> Vec<StorageRoot>;  // pure, testable
pub fn usage(path: &Path) -> Option<Usage>;                                  // rustix::fs::statvfs
pub fn used_fraction(u: Usage) -> Option<f64>;                               // None when total == 0, else clamp 0..1
```

### 3.8 Job registry (shared with area:26 and area:21/22)

```rust
// src/jobs/registry.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState { Enqueued, Running, Succeeded, Failed, Cancelled, Blocked }

#[derive(Clone, Debug)]
pub struct JobInfo {
    pub id: uuid::Uuid,             // add `uuid` crate, feature v4
    pub tags: Vec<String>,
    pub state: JobState,
    pub next_run: Option<chrono::DateTime<chrono::Utc>>,
    pub attempts: u32,              // runAttemptCount; display shows attempts + 1
}

pub struct JobRegistry {
    jobs: std::sync::Mutex<Vec<JobInfo>>,                 // one lock covers busy check + enqueue (RACE-06)
    snapshot: tokio::sync::watch::Sender<Vec<JobInfo>>,   // pushed after every transition
}

impl JobRegistry {
    pub fn enqueue_unique(&self, name: &str, job: impl Future<Output = JobOutcome> + Send + 'static, policy: UniquePolicy) -> JobId;
    pub fn is_running(&self, tag: &str) -> bool;
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<Vec<JobInfo>>;
    pub fn cancel_unique(&self, name: &str);
}

pub enum UniquePolicy { Keep, Replace }  // Keep = WorkManager ExistingWorkPolicy.KEEP
pub enum JobOutcome { Success, Retry, Failure }
```

`enqueue_unique(..., Keep)` returns the existing id and drops the new future when a job with the same name is ENQUEUED or RUNNING, matching Kotlin `KEEP`.

---

## 4. Async and threading model

| Thread | Owns | Runs |
|---|---|---|
| GTK main thread | all widgets, page models (`Rc<RefCell<..>>`), `SettingsShell` | event handlers, rendering, `spawn_future_local` consumers |
| tokio runtime (multi-thread, created in `main.rs`) | IO | HTTP fetch, SQLite via `spawn_blocking`, statvfs, job bodies |
| rusqlite | one `Connection` per worker behind a `Mutex`, or a dedicated thread with a channel (proposal: dedicated thread, simplest correct) | queries |

Pattern (one per page):

```rust
// inside a page constructor, GTK thread
let (tx, rx) = async_channel::unbounded::<RepoMsg>();
let model = Rc::new(RefCell::new(MangaScreenState::Loading));
glib::spawn_future_local(clone!(@strong model => async move {
    while let Ok(msg) = rx.recv().await {
        apply(&mut model.borrow_mut(), msg);   // pure reducer, unit-tested
        render(&model.borrow());               // GTK updates only here
    }
}));
// IO side: spawn on tokio, send results back through tx
let tx2 = tx.clone();
RUNTIME.spawn(async move {
    let rows = store.list_manga_repos().await;     // spawn_blocking inside
    let _ = tx2.send(RepoMsg::Loaded(rows)).await;
});
```

Rules:
- `apply` is a pure function `fn(&mut State, Msg)`. All state transitions from section 5 live there, so they are testable without GTK.
- Subscriptions (the DB list "flow") are a tokio task that loops on a `rusqlite` change notification (proposal: a `tokio::sync::watch` the store writes after every mutation) and sends `Loaded(rows)`. Dropping the page drops `rx`; the send then fails and the task ends.
- Deep-link queue: `Option<PendingDeepLink>` in the page; apply it on the first `Loaded` message (fixes the Kotlin race, section 5.3).
- Never call `gtk::` from tokio tasks. Never block the GTK thread on IO (fixes the Kotlin synchronous validation).

---

## 5. Integration with the mockup

### 5.1 Dispatch

Replace arms of `settings::body()` (`/home/blackbox/kurobayomi/src/settings.rs:7-24`) that belong to this area:

```rust
pub fn body(id: &str) -> Option<Built> {
    match id {
        "settings" => shell::main_menu(),                                  // was main_menu()
        "settings_search" => search(),                                     // unchanged, part 1 / 47b
        "settings_app_language" => app_language::page(),                   // was language()
        "settings_anime_extension_stores" => repos::anime::page(),         // was cards(id, &["Store A","Store B"], false)
        "settings_manga_extension_repos" => repos::manga::page(),          // was cards(id, &["Repo A"], false)
        "settings_create_backup" => backup::create::page(),                // new (no arm in mockup yet)
        "settings_restore_backup" => backup::restore::page(),              // new
        "settings_worker_info" => debug::workers::page(),                  // was text_dump(...)
        "settings_backup_schema" => debug::schema::page(),                 // was text_dump(...)
        "settings_debug_info" => debug::info::page(),                      // new
        _ => /* part-1 and other arms unchanged */ None,
    }
}
```

Keep `cards()` and `text_dump()` for the part-1 screens that still use them (player custom buttons, licenses, clear DB). Do not delete them.

### 5.2 Spec

`/home/blackbox/kurobayomi/spec/settings.json` remains the source of truth for titles, toolbar labels and dialog ids. The Rust pages read titles from the same constants, not from spec, for the strings that the spec gets wrong. Known spec bugs to fix in the same change (owned by this area):
- `settings_clear_anime_database` toolbar lists `"Select all"` twice; Aniyomi's order is Select all / Select inverse (part-1 page, flagged here because it is in the same spec file).
- `settings_manga_extension_repos` dialog `add_repo` body hard-codes "Add additional repos to Aniyomi."; the Kotlin source uses `AYMR.strings.action_add_repo_message` with `%s` = app name. Substitute the Kurobayomi name at build time.
- `settings_create_backup` note says the create button is "pinned"; the Kotlin pins it inside `LazyColumnWithAction` (bottom, outside scroll). Keep.

### 5.3 Dialog model in the mockup

The mockup opens dialogs with `dialog_of(id, name)` and `show_dialog(d)` from the spec. Keep that for the dialogs whose content is fixed. The stateful dialogs (create with live duplicate check, deep-link confirm with processing state) are built in `repos/dialogs.rs` as GTK windows because they need entry signals, processing state and error text. `show_dialog` is still used by part-1 pages.

---

## 6. Android API to Linux mapping (this area)

| Android / Kotlin | Used in | Linux / Rust replacement | Notes |
|---|---|---|---|
| Voyager `Screen`, `Navigator`, `LocalNavigator` | all pages | `SettingsShell` with `gtk::Stack` history (`Vec<PageId>`) | `back()` semantics in section 3.1 |
| Voyager `StateScreenModel`, `screenModelScope` | repos, backup | page model struct + `spawn_future_local` consumer (section 4) | no lifecycle framework needed |
| `rememberScreenModel` | repos, backup, worker | created in page constructor, dropped on page close | |
| `Injekt.get()` default args | models | explicit constructor parameters (`Arc<Store>`, `Arc<Extensions>`) | testable |
| `StateFlow.update {}` | models | single-owner `RefCell` on GTK thread | no atomics needed |
| `Channel(Int.MAX_VALUE)` events | manga repos | `async_channel::unbounded` | |
| `LaunchedEffect(url)` deep link | repos | `Option<PendingDeepLink>` applied on `Loaded` | fixes race |
| `AlertDialog` (Material 3) | dialogs | `gtk::AlertDialog` for text-free confirms (delete, conflict, confirm-add); `gtk::Window` (transient, modal) for dialogs with an entry | `gtk::AlertDialog` requires GTK 4.10+ |
| `OutlinedTextField` | create dialogs | `gtk::Entry` with `set_input_hints(InputHints::NO_SPELLCHECK)` and `set_input_purpose(InputPurpose::Url)` | purpose = URL keyboard hint on touch |
| `TextFieldState`, `derivedStateOf` | anime create | `connect_changed` on `gtk::Entry` recomputes `already_exists` | |
| `LaunchedEffect` + `delay(0.1 s)` + `requestFocus()` | create dialogs | `gtk::Window::present()` then `entry.grab_focus()` in `connect_map` | no timer |
| `ElevatedCard` | store/repo cards | `gtk::Box` with CSS class `card` (theme-driven) | do not hard-code colours |
| `IconButton` | card actions | `gtk::Button::from_icon_name` with `set_tooltip_text` (content description) | icon names in section 7 |
| `LazyColumn` | lists | `gtk::ListBox` with `selection-mode = none` inside `gtk::ScrolledWindow` | |
| `EmptyScreen` | empty state | `gtk::Box` centred with symbolic icon + `dim-label` | |
| Extended FAB + `shouldExpandFAB` | page FAB | `gtk::Button` `suggested-action` + `pill` (mockup already does); collapse to icon-only on scroll-forward via `gtk::Adjustment` value-changed | |
| `Scaffold` + `AppBar` | pages | `gtk::HeaderBar` (GTK's own, theme-drawn) per page in the shell | not libadwaita |
| `LazyColumnWithAction` | create, restore | `gtk::Box` vertical: scroller (expand) + `gtk::Separator` + full-width `suggested-action` button | |
| `SelectionContainer` | restore error | `gtk::Label` with `set_selectable(true)` | |
| `LinearProgressIndicator` | storage | `gtk::ProgressBar` with `set_fraction` | hide when fraction is `None` |
| `Formatter.formatFileSize` | storage | `util::fmt_size::format(bytes)` (own SI implementation) | section 7.4 of behaviour spec |
| `ClipboardManager` | copy actions | `gdk::Display::default()?.clipboard().set_text(s)` | |
| `Toast` | all toasts | `toast::Overlay` (gtk::Overlay + gtk::Revealer, 2 s) | no libadwaita |
| `ActivityResultContracts.CreateDocument` | create | `gtk::FileDialog::save(...)` with `set_initial_name` | portal-backed on Flatpak/KDE |
| `ActivityResultContracts.OpenDocument` (restore entry, part 1) | restore | `gtk::FileDialog::open(...)` | filter `*.tachibk` |
| `takePersistableUriPermission` | create/restore | not needed (paths) | dropped |
| `WorkManager` / `WorkInfo` / `WorkQuery` | worker info, job enqueue | `jobs::registry::JobRegistry` | section 3.8 |
| `workManager.isRunning(tag)` | create busy check | `JobRegistry::is_running(tag)` | |
| `DeviceUtil`, `Build.*` | debug info | `/sys/class/dmi/id/*`, `/etc/os-release`, `rustix::system::uname` | |
| `WebViewUtil.getVersion` | debug info | `build.rs` `pkg-config --modversion webkitgtk-6.0` | value fixed at build; fine |
| `ProfileVerifier` | debug info | dropped | |
| `BuildConfig.*` | about, debug | `env!`, `build.rs` env vars | section 8 of data spec |
| `java.time` `LocalDateTime`, `DateTimeFormatter` | worker info, build time | `chrono` | default format proposal in section 7 |
| `kotlinx.serialization` `ProtoBufSchemaGenerator` | backup schema | `include_str!("../proto/backup.proto")` | |
| `Locale`, `LocaleListCompat`, `AppCompatDelegate` | app language | ICU display names + settings key; apply at startup via `LANGUAGE` env / gettext | restart required |
| `android:localeConfig` XML | app language | `build.rs` lists translation folders | section 4 of data spec |
| `Intent`, `ContentResolver`, `Uri` | create/restore | `std::path::PathBuf`, `gio::File` where a portal returns a URI | |
| `copyToClipboard` toast on API <= 32 | copy actions | our own toast with `truncate_center(50)` | |

---

## 7. Icon names (freedesktop, for `icon_name`)

| Android icon | Used for | Icon name |
|---|---|---|
| `Icons.AutoMirrored.Outlined.Label` | store/repo name row | `tag-symbolic` (mockup already uses) |
| `Icons.Outlined.Public` | open website (anime) | `web-browser-symbolic` |
| `Icons.AutoMirrored.Outlined.OpenInNew` | open website (manga) | `external-link-symbolic` (fallback `web-browser-symbolic`) |
| `CustomIcons.Discord` | Discord link | no freedesktop Discord icon exists; proposal: bundled symbolic SVG `assets/icons/discord-symbolic.svg` loaded with `gtk::Image::from_file` (or `gtk::gio::BytesIcon`) |
| `Icons.Outlined.ContentCopy` | copy | `edit-copy-symbolic` |
| `Icons.Outlined.Delete` | delete | `user-trash-symbolic` |
| `Icons.Outlined.Refresh` | refresh | `view-refresh-symbolic` |
| `Icons.Default.Check` | selected language | `object-select-symbolic` |
| `Icons.Outlined.Add` | FAB | `list-add-symbolic` |
| `Icons.Default.ContentCopy` (debug) | copy in debug pages | `edit-copy-symbolic` |

---

## 8. Error handling

- Library modules return `Result<T, E>` with `thiserror` enums. `RepoError`, `BackupValidationError`, `StorageError`, `JobError`.
- User-visible text is produced at the page layer only. Keep the Kotlin fallback texts (`"unknown error"` for anime add) so behaviour matches; log the typed error with `tracing::error!`.
- No `unwrap()` on IO. The Kotlin `!!` in `AnimeExtensionStoreService.getExtensions` is not in scope and must not be copied.
- Panics: none expected. `apply()` reducer is total over its enums.

---

## 9. Pitfalls

1. **Synchronous validation**: the Kotlin restore validator decodes the whole file on the thread that creates the page. Rust must not. Show `Validating` and disable Restore.
2. **Deep-link race**: the Kotlin drops the link if it arrives while Loading. Queue it (section 4).
3. **Dialog reset**: the Kotlin manga model resets `dialog` on every DB emission. In Rust the reducer must keep the dialog on `Loaded`, except when the dialog's own target row disappears (proposal). Test this (`p2-edge-cases-and-tests.md`).
4. **Silent KEEP drop**: restore has no busy check. Add one; toast `backup_in_progress`.
5. **Progress > 1**: never pass unclamped fractions to `gtk::ProgressBar` (GTK clamps silently, hiding the bug). Compute with `used_fraction`.
6. **Blocking the GTK thread on `statvfs`**: `statvfs` is fast, but run it in the `Loaded` path via tokio anyway to keep the UI rule simple.
7. **gtk::FileDialog portal**: on KDE, the portal may return a `file://` URI; convert with `gio::File::path()`. On sandbox without portal, the call errors; map to `file_chooser.unavailable`.
8. **Clipboard on Wayland**: the clipboard content is owned by the process; it disappears when Kurobayomi exits. Acceptable, same as Android semantics.
9. **Escape and Enter**: GTK window default-response handling. Set `set_default_widget` to the primary button on create dialogs. Escape closes via `gtk::Window` default behaviour; verify with the shortcut controller.
10. **Locale display names**: ICU output differs from Java in some languages (e.g. capitalisation of the first word). Accept, document.
11. **Mount scan on NFS or FUSE**: `fuseblk` entries can hang on `statvfs` if the device is unresponsive. Run statvfs with a 2 s tokio timeout; on timeout show the root without usage text.
12. **Schema text drift**: `proto/backup.proto` must be the one the decoder uses (area:21). A test compares the field numbers of the proto with the decoder's field tags.

---

## 10. Testing hooks

- Pure functions: `two_pane`, `BackupOptions::{to_array, from_array, can_create}`, `RestoreOptions::can_restore`, `validate`, `storage_roots`, `used_fraction`, `format` (size), `language_list`, `worker_dump` (text format), `mangarepo_url_check`.
- Reducers: `apply` for each page (Loading, Ready, dialog transitions, deep-link queue, event emission).
- Fakes: `SourceCatalog`, `TrackerCatalog`, in-memory `Store`, fake `Fetcher`.
- Fixtures under `tests/fixtures/settings/` (see `p2-edge-cases-and-tests.md` section 4).

---

## 11. What is intentionally not designed here

- The backup encode and decode path, the index body parsing, the extension store fetch: owned by area:21, area:22, area:18, area:19. This area consumes their Rust APIs.
- Notifications for backup progress: area:22 (`BackupNotifier` equivalent).
- The part-1 settings pages (Main, Data, Tracking, Appearance, About, Clear DB, Licenses).
