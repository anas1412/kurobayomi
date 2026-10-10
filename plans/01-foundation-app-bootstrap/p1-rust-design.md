# Area 01 — Rust design

Targets: stable Rust 2024 edition, gtk4-rs 0.11 (`gtk4` crate, feature `v4_16`, no libadwaita), tokio, rusqlite, reqwest, serde. The current `Cargo.toml` of `/home/blackbox/kurobayomi` has only `gtk4`, `serde`, `serde_json`; this design adds crates workspace by workspace as tasks land (each task lists what it adds).

## 1. Workspace layout

```
kurobayomi/
  Cargo.toml                  workspace root + the existing GUI package "kurobayomi" (src/*.rs)
  build.rs                    none (build info lives in kuro-core)
  src/                        GUI crate (existing mockup): main.rs ui.rs screens.rs settings.rs entry.rs player.rs clamp.rs spec.rs
    cli.rs                    NEW  argv parsing (Cli, DeepLink)
    crash_window.rs           NEW  reporter mode window
    incognito_indicator.rs    NEW
    theme_mode.rs             NEW
    prefs_bind.rs             NEW  PrefBinding, bind()
    widgets/tri_check.rs      NEW  tri-state check button
    lifecycle.rs              NEW  ProcessLifecycle
  crates/
    kuro-core/                no GTK, no OS-specific code
      build.rs                git info -> env (p1-topic-build-and-dependency-map.md 1.3)
      src/lib.rs
      src/app_info.rs         AppInfo, BUILD constants
      src/app_context.rs      AppContext (+ accessor per service)
      src/bootstrap.rs        run_startup(): logging, crash, settings, runtime, context
      src/crash.rs            hook, report file, CrashContext, contain()
      src/crash_dump.rs       debug info, extension blocks, log tail, dump writer
      src/logging.rs          tracing setup
      src/migrate/mod.rs      Version, Migration, Strategy, select, plan, run, run_blocking
      src/migrate/list.rs     MIGRATIONS static
      src/db.rs               Db::open, PRAGMAs, user_version hook
      src/db_types.rs         UnixMillis, StringList, UpdateStrategy, FetchType, Memo (ToSql/FromSql)
      src/notify.rs           NotificationCategory table, ids
      src/links.rs            URL_HELP, URL_HELP_UPCOMING
      src/deep_link.rs        DeepLink enum + parse/format
      src/util/collections.rs insert_separators, insert_separators_reversed
      src/sources_gate.rs     SourceGate (is_initialized watch)
    kuro-platform/            the only crate with cfg(target_os) / D-Bus / libc (rule 1 of the cross-platform addendum)
      src/lib.rs              traits Paths, SecretStore, Notifier, MediaControls, WebViewHost, Opener, IdleInhibitor, SingleInstance, SchedulerBackend, FileAssociations, Autostart, ExternalPlayer, AppLock + spawn_detached
      src/linux/*.rs          implementations
    kuro-testkit/             in-memory pref store, temp-dir Paths, FakeTracker, fixtures
```
Existing mockup modules stay where they are. New app crates for other areas (`kuro-data`, `kuro-domain`, `kuro-source`, `kuro-source-local`, `kuro-i18n`) are created by their areas and depend on `kuro-core`; none of them depends on the GTK crate.

## 2. Crates (and why)

| Crate | Where | Why |
|---|---|---|
| `tokio` (`rt-multi-thread`, `sync`, `time`, `macros`) | kuro-core | async runtime, matches the user's stack choice |
| `tracing`, `tracing-subscriber` (`env-filter`, `fmt`, `reload`), `tracing-appender` | kuro-core | structured logging with level filters, daily file rotation (replaces `logcat`) |
| `anyhow`, `thiserror` | all | `thiserror` for library error enums in crates, `anyhow` at app edges |
| `rusqlite` (`bundled`) | kuro-core::db | same reasoning as requery SQLite: pinned SQLite version; `deadpool-sqlite` or hand-written pool decided in area:05 |
| `serde`, `serde_json`, `toml` | kuro-core | settings, crash facts, Json defaults of `AppModule.kt:173-178` |
| `time` (`formatting`, `local-offset`) | kuro-core | RFC 3339 with local offset (replaces `OffsetDateTime.now(ZoneId.systemDefault())`) |
| `directories` | kuro-platform | XDG paths behind `Paths` |
| `rustix` (`system`, `fs`) | kuro-platform | `uname`, `/proc` reads without `libc` unsafe blocks |
| `async-channel` | GUI crate | UI-thread mailbox for results from tokio |
| `futures` | kuro-core | `BoxFuture` for migrations |
| `gtk4`, `gio`, `glib` (via `gtk4`) | GUI crate | UI |
| `pulldown-cmark`, `ttf-parser`, `scraper`, `quick-xml`, `prost`, `image`, `rquickjs`, `webkit6`, `libmpv2`, `souvlaki`, `reqwest`, `hickory-resolver` | other areas | see `p1-topic-build-and-dependency-map.md` section 2 |

No `lazy_static`/`once_cell`: `std::sync::{OnceLock, LazyLock}` and `std::cell::{OnceCell, LazyCell}` cover everything (stable since Rust 1.80).

## 3. Key types and signatures

### 3.1 `AppInfo`
```rust
// kuro-core/src/app_info.rs
pub struct AppInfo {
    pub app_id: &'static str,                 // "io.github.anas1412.Kurobayomi"
    pub version_name: &'static str,           // env!("CARGO_PKG_VERSION"), plus "-<commit count>" in debug (built in const via concat in build.rs output)
    pub version_code: u32,                    // VERSION_CODE
    pub commit_count: &'static str,
    pub commit_sha: &'static str,
    pub build_time: &'static str,
    pub build_type: BuildType,
    pub updater_enabled: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildType { Debug, Preview, Release }
pub static APP_INFO: AppInfo = AppInfo { /* env!("KURO_*") values */ };
impl AppInfo {
    pub fn supported_image_mime_types(&self) -> Vec<&'static str>;   // order: avif, gif, heif, jpeg, jxl, png, webp; filtered by compiled decoders
    pub fn is_debug(&self) -> bool { self.build_type == BuildType::Debug }
}
pub const VERSION_CODE: u32 = 1;
```

### 3.2 Preference binding (GUI crate)
Assumes the contract of area:02-preferences-system:
```rust
pub trait Pref<T: Clone + Send + Sync + 'static>: Send + Sync {
    fn key(&self) -> &str;
    fn get(&self) -> T;                       // never fails; invalid stored value -> delete + default (AndroidPreference.get)
    fn set(&self, value: T);
    fn is_set(&self) -> bool;
    fn delete(&self);
    fn default_value(&self) -> T;
    fn changes(&self) -> tokio::sync::watch::Receiver<T>;   // latest value, conflated; receiver already holds the current value
}
```
```rust
// src/prefs_bind.rs
pub struct PrefBinding<T: Clone + 'static> {
    pref: Arc<dyn Pref<T>>,
    value: Rc<RefCell<T>>,
    _task: glib::JoinHandle<()>,
}
impl<T: Clone + Send + Sync + 'static> PrefBinding<T> {
    pub fn new(pref: Arc<dyn Pref<T>>) -> Self {
        let value = Rc::new(RefCell::new(pref.get()));
        let mut rx = pref.changes();
        let v2 = value.clone();
        let task = glib::MainContext::default().spawn_local(async move {
            while rx.changed().await.is_ok() { *v2.borrow_mut() = rx.borrow_and_update().clone(); }
        });
        Self { pref, value, _task: task }
    }
    pub fn get(&self) -> T { self.value.borrow().clone() }
    pub fn set(&self, v: T) { self.pref.set(v) }          // write-through: the mirror changes when the notification arrives (PreferenceMutableState semantics)
}
/// Call `f` now with the current value and again after every change (Kotlin: changes().onEach{}.launchIn(scope) with the "ignition" emission).
pub fn bind<T: Clone + Send + Sync + 'static>(pref: Arc<dyn Pref<T>>, f: impl Fn(&T) + 'static) -> glib::JoinHandle<()>;
```
Dropping the `PrefBinding` aborts its task (equivalent of cancelling the `CoroutineScope`). `bind` returns the handle so widgets can store it or call `.abort()`; widget `connect_destroy` should abort bindings it created.

### 3.3 Tri-state check
```rust
// kuro-core/src/prefs/tri_state.rs  (type owned by area:02; shown for the mapping)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckState { None, Include, Exclude }
impl CheckState { pub fn next(self) -> Self { match self { Self::Exclude => Self::None, Self::Include => Self::Exclude, Self::None => Self::Include } } }

// src/widgets/tri_check.rs
impl CheckState { pub fn gtk(self) -> (/*active*/ bool, /*inconsistent*/ bool) { match self { Self::Include => (true, false), Self::Exclude => (false, true), Self::None => (false, false) } } }
pub fn tri_check(initial: CheckState, on_change: impl Fn(CheckState) + 'static) -> gtk::CheckButton;
```
`tri_check` suppresses GTK's own toggling (`connect_toggled` handler resets via `set_active`/`set_inconsistent` from the model) and uses `GestureClick` + `Space`/`Enter` activation to call `next()`.

### 3.4 Source gate
```rust
// kuro-core/src/sources_gate.rs
pub trait SourceGate { fn is_initialized(&self) -> tokio::sync::watch::Receiver<bool>; }   // implemented by MangaSourceManager and AnimeSourceManager (area:17)
// GUI crate: show spinner until true
pub fn when_sources_ready(gate: &dyn SourceGate, stack: &gtk::Stack, loading: &gtk::Widget, content: impl Fn() -> gtk::Widget + 'static);
```

### 3.5 Collections
The generator borrows `before` and `after`, so the implementation works with indices:
```rust
pub fn insert_separators<T, R: From<T>>(items: Vec<T>, mut gen: impl FnMut(Option<&T>, Option<&T>) -> Option<R>) -> Vec<R> {
    let n = items.len();
    if n == 0 { return Vec::new(); }
    let mut seps: Vec<Option<R>> = Vec::with_capacity(n + 1);
    for i in 0..=n {                                   // i = index of the "after" element; before = i-1
        let before = i.checked_sub(1).and_then(|b| items.get(b));
        let after = items.get(i);
        seps.push(gen(before, after));                 // call order: (None, items[0]), then each adjacent pair in order, last call (items[n-1], None)
    }
    let mut out = Vec::with_capacity(2 * n + 1);
    let mut seps = seps.into_iter();
    out.extend(seps.next().flatten());                 // separator before the first item
    for item in items {
        out.push(R::from(item));
        out.extend(seps.next().flatten());             // separator after this item
    }
    out
}
pub fn insert_separators_reversed<T, R: From<T>>(items: Vec<T>, mut gen: impl FnMut(Option<&T>, Option<&T>) -> Option<R>) -> Vec<R> {
    // same layout; the generator is invoked from the end: first call (items[n-1], None), last call (None, items[0])
    let n = items.len();
    if n == 0 { return Vec::new(); }
    let mut seps: Vec<Option<R>> = (0..=n).map(|_| None).collect();
    for i in (0..=n).rev() {
        let before = i.checked_sub(1).and_then(|b| items.get(b));
        let after = items.get(i);
        seps[i] = gen(before, after);
    }
    let mut out = Vec::with_capacity(2 * n + 1);
    let mut seps = seps.into_iter();
    out.extend(seps.next().flatten());
    for item in items { out.push(R::from(item)); out.extend(seps.next().flatten()); }
    out
}
```
(The `R: From<T>` bound is the Rust spelling of Kotlin's `T : R`; call sites with a single enum `UiModel { Item(T), Header(String) }` implement `From<T>`.) Everything else in `CollectionUtils.kt` is replaced by std (`HashSet::insert`/`remove`, `Iterator::filter`, `Iterator::partition`, `Iterator::filter().count()`); no wrapper functions are written.

### 3.6 Database open
```rust
// kuro-core/src/db.rs
pub struct DbSpec { pub file_name: &'static str, pub schema_version: u32, pub create: fn(&Connection) -> rusqlite::Result<()>, pub upgrade: fn(&Connection, from: u32, to: u32) -> rusqlite::Result<()> }
pub struct Db { /* pool of connections */ }
impl Db {
    pub fn open(path: &Path, spec: &DbSpec) -> Result<Db, DbError>;
    pub async fn read<R: Send + 'static>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<R> + Send + 'static) -> Result<R, DbError>;
    pub async fn write<R: Send + 'static>(&self, f: impl FnOnce(&Transaction) -> rusqlite::Result<R> + Send + 'static) -> Result<R, DbError>;
}
pub(crate) fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;       // returns a row; pragma_update tolerates it
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(Duration::from_millis(5000))?;        // addition, absent in Aniyomi: WAL writers wait instead of failing with SQLITE_BUSY
    Ok(())
}
```
Open sequence (equivalent of the SQLDelight driver callback): create parent directory; open connection; `configure`; read `PRAGMA user_version`; `0` and no tables → run `spec.create` and set `user_version = schema_version` inside one transaction; `v < schema_version` → `spec.upgrade(v, schema_version)` in one transaction, then set `user_version`; `v > schema_version` → `DbError::TooNew { found, supported }` (the database was written by a newer app; the crash screen shows the message `Database is newer than this version of Kurobayomi (found {found}, supported {supported})`). Schemas themselves come from area:03 and area:04.

### 3.7 `NotificationCategory`
```rust
pub struct NotificationCategory { pub channel_id: &'static str, pub group: Option<&'static str>, pub importance: Importance, pub name_key: &'static str, pub show_badge: bool, pub silent: bool }
pub enum Importance { Low, Default, High }
// The 13 rows of p1-data-and-config.md section 9, in that order: common_channel, library_progress_channel, library_errors_channel,
// new_chapters_episodes_channel, downloader_progress_channel, downloader_error_channel, backup_restore_progress_channel,
// backup_restore_complete_channel_v2, incognito_mode_channel, torrent_server_channel, http_server_channel,
// app_apk_update_channel, ext_apk_update_channel.
pub const CATEGORIES: [NotificationCategory; 13] = [/* one struct literal per channel id listed above */];
pub fn key(channel_id: &str, id: i32) -> String { format!("{channel_id}:{id}") }
```
Mapping to freedesktop (implemented in `kuro-platform::linux::notifier`): `Importance::Low` becomes urgency `low`; `Default` and `High` become urgency `normal`, and `High` additionally sets the `resident` hint false and asks the desktop for a heads-up (`x-kde-display-appname` is not needed); `silent` sets `suppress-sound = true`. The spec `category` hint: `transfer` for the progress/complete channels of library, downloader and backup, `transfer.error` for the two error channels, `device` for the torrent and http server channels, none for the others.

### 3.8 `DeepLink`
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeepLink { AnimeLibrary, Library, Manga(i64), Anime(i64), Updates, History, Sources, AnimeExtensions, Extensions, AnimeDownloads, Downloads }
impl DeepLink {
    pub fn parse(arg: &str) -> Option<DeepLink>;            // accepts "manga=12", "anime=3", "updates", "history", "library", "anime-library", "sources", "extensions", "anime-extensions", "downloads", "anime-downloads"
    pub fn name(&self) -> &'static str;                     // the same names as accepted by parse, without the "=<id>" part
    pub fn action_name(&self) -> String;                    // "app.open-manga"
}
```
`Manga(id)`/`Anime(id)` parse requires `=<i64>`; without id the Kotlin code returns false (not handled), so `parse("manga")` returns `None`.

## 4. Error handling policy
* Library crates return `Result<T, TheirError>` (`thiserror`). The GUI crate converts to user-visible messages; messages are string keys (area:51).
* `panic!` is reserved for programmer errors and unrecoverable startup failures (database cannot be opened, settings directory not creatable): they reach the crash window by design (parity with Kotlin throwing in `AppModule`).
* Foreign untrusted input (images, archives, JSON from sources) never panics the app: `crash::contain` around third-party decoders; parse failures are `Err`.
* Logging policy: `error!` for conditions that should not happen, `warn!` for recoverable external failures, `info!` for lifecycle events, `debug!`/`trace!` for verbose mode.

## 5. Android API → Linux mapping for the symbols in this scope

| Android/Kotlin API (file) | Linux/Rust replacement |
|---|---|
| `Application`, `Application.onCreate` (`App.kt`) | `main()` + `gtk::Application::connect_startup/activate` |
| `Application.getPackageName` override (`App.kt:222-236`) | Drop (T-01a-026): WebKitGTK sends no `X-Requested-With`; the host-side header policy belongs to area:12 and area:36 (WebView) |
| `ProcessLifecycleOwner`, `DefaultLifecycleObserver`, `lifecycleScope` | `ProcessLifecycle` + `glib::MainContext::spawn_local` |
| `SingletonImageLoader.Factory`, Coil `ImageLoader` | `ImagePipeline` built in `AppContext` (area:34) |
| `Thread.setDefaultUncaughtExceptionHandler` | `std::panic::set_hook` |
| `Intent`, `PendingIntent`, `BroadcastReceiver`, `IntentFilter`, `ContextCompat.registerReceiver` | `gio::SimpleAction` + `DeepLink`; no receiver concept |
| `NotificationManagerCompat`, `NotificationCompat.Builder`, channels | `Notifier` trait (gio notifications), `NotificationCategory` table |
| `Context.startActivity(Intent(ACTION_SEND))` chooser | clipboard + `Opener::reveal` |
| `FileProvider` (`getUriCompat`) | none; plain paths |
| `Runtime.getRuntime().exec("logcat *:E -d -v year -v zone -f <file>")` | read the tracing log file tail |
| `android.os.Build` fields RELEASE, SDK_INT, DISPLAY, BRAND, MANUFACTURER, DEVICE, PRODUCT, MODEL (`CrashLogUtil.getDebugInfo`) | `/etc/os-release`, `uname`, `XDG_*` env |
| `WebView.setDataDirectorySuffix`, `WebViewUtil` | Drop for the suffix; WebKitGTK version string for the dump |
| `AppCompatDelegate.setDefaultNightMode` | `gtk::Settings::set_gtk_application_prefer_dark_theme` (SYSTEM restores the initial value) |
| `WindowCompat.setDecorFitsSystemWindows` (`CrashActivity`) | none (normal window) |
| `finishAffinity` + `startActivity(MainActivity)` | spawn detached self + `quit()` |
| `javax.microedition.khronos.egl` (`GLUtil`) | `GL_MAX_TEXTURE_SIZE` probe through `gdk::GLContext` (T-01a-022) |
| `ActivityManager.MemoryInfo.totalMem` (`DeviceUtil.isLowRamDevice`) | `/proc/meminfo` `MemTotal` |
| `Settings.Global.ANIMATOR_DURATION_SCALE` | `gtk::Settings::is_gtk_enable_animations()` |
| `Conscrypt`, `Security.insertProviderAt` | rustls (reqwest) |
| `SharedPreferences` | `Settings` (area:02) |
| `ContextCompat.getMainExecutor(app).execute` | `glib::idle_add_local_once` |
| `Dispatchers.IO.limitedParallelism(n)` | `tokio::sync::Semaphore::new(n)` |
| `kotlinx.serialization.json.Json { ignoreUnknownKeys = true; explicitNulls = false }` | serde with `#[serde(default)]` on every field and `skip_serializing_if = "Option::is_none"` on every `Option` (the combination reproduces "ignore unknown keys" (serde default) and "do not write nulls") |
| `XML { ignoreUnknownChildren; indent = 2; xmlDeclMode = Charset; XML10 }` | `quick-xml` writer with indent 2 spaces, declaration `<?xml version="1.0" encoding="UTF-8"?>`; unknown children ignored by serde default |
| `ProtoBuf` default | `prost` |
| `Injekt` | `AppContext` |
| `rx.Observable.asFlow()` | none |
| `Date`, `OffsetDateTime` | `time::OffsetDateTime`, epoch milliseconds (`i64`) in the database |

## 6. Serialization singletons (AppModule `Json`, `XML`, `ProtoBuf`)
There are no shared singletons in Rust; the contract is carried by conventions that area:21-backup-format-and-create and area:14-source-local-and-metadata follow:
* JSON (`explicitNulls = false` means null fields are omitted when encoding, and missing fields are accepted when decoding; `ignoreUnknownKeys = true`): every serde struct that mirrors an Aniyomi JSON document has `#[serde(default)]`, `#[serde(skip_serializing_if = "Option::is_none")]` on optionals, and no `deny_unknown_fields`.
* XML (ComicInfo.xml): `quick-xml` serializer configured with indent of two spaces and a declaration that states the encoding (the Kotlin `XmlDeclMode.Charset` writes `<?xml version='1.0' encoding='UTF-8'?>`-style declarations; the exact quote style is irrelevant to readers); unknown children are ignored (serde default). `autoPolymorphic = true` has no Rust counterpart because ComicInfo models are concrete structs.
* Protobuf: the backup schema from area:21 compiled with `prost-build`; field numbers must match Aniyomi exactly.

## 7. Pitfalls
* GTK is not `Send`: `PrefBinding`, `IncognitoIndicator`, `ProcessLifecycle` live on the main thread; the `Pref<T>` they wrap is `Send + Sync` and may be written from tokio tasks.
* `watch::Receiver::changed()` collapses multiple updates into one; this equals the Kotlin `.conflate()` in `AndroidPreference.changes()`, but code that must see every transition (none in this area) must not use preferences for that.
* `gtk::Settings::default()` returns `None` before GTK init (headless tests); `theme_mode::apply` checks and returns.
* `std::process::exit` inside the panic hook is intentional; do not replace it with `abort()` (core dumps are noisy on systemd-coredump setups).
* Keep `src/main.rs` free of services: it constructs the context and passes it down; widgets obtain services via `ui::ctx()`.
* The mockup reads `spec/*.json` at runtime through `thread_local! SPEC` (`src/ui.rs`); do not move that into `AppContext`: the spec is a design artifact and disappears as screens become real.
