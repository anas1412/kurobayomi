# Topic: AppContext (the DI replacement), startup order, threading, incognito indicator

Context: Aniyomi wires everything with Injekt (`p1-behavior.md` section 13). The cross-platform addendum (`plans/addenda/requirements-cross-platform.md`, rule 3) says OS traits reach application crates "through the app context (the same place the Injekt/DI replacement from area:01-foundation-app-bootstrap lives), never through globals". This document defines that app context.

## 1. Decision: no DI container

Injekt is a service locator keyed by type. In Rust the same job is done by one plain struct whose fields are the singletons. Reasons (ladder: does it need to exist at all? no):
* The set of singletons is closed and known at compile time (about 45, listed in section 2).
* A `HashMap<TypeId, Box<dyn Any>>` locator loses compile-time checking and makes "missing registration" a runtime panic; a struct field cannot be forgotten.
* Tests construct the services they need directly, exactly as Aniyomi tests do with `InMemoryPreferenceStore`; no registry reset between tests.

Rule: a service receives its collaborators as constructor arguments (`Arc<T>` or a small trait object), never `ctx` as a whole, except top-level UI code that needs several services. This keeps crates acyclic: `kuro-core` does not know the GTK crate.

## 2. `AppContext` fields (maps 1:1 to Injekt registrations)

Legend for "Build": E = built eagerly in `AppContext::new`, L = lazily on first access through `OnceLock` (thread safe, equivalent to the Mihon `patchInjekt` fix), W = also forced by the warm-up list (section 4).

| Field | Aniyomi registration | Build | Owner area |
|---|---|---|---|
| `platform: Arc<dyn Platform>` (bundle of `Paths`, `SecretStore`, `Notifier`, `Opener`, `MediaControls`, `WebViewHost`, `IdleInhibitor`, `SingleInstance`, `SchedulerBackend`, `FileAssociations`, `Autostart`, `ExternalPlayer`, `AppLock`; trait list owned by `plans/addenda/requirements-cross-platform.md`) | `addSingleton(app)` (the Android `Application` as service locator for Context) | E | `kuro-platform` |
| `info: &'static AppInfo` | `BuildConfig` + `AppInfo` | E | this area |
| `rt: tokio::runtime::Handle` | `Dispatchers.IO/Default` | E | this area |
| `settings: Arc<Settings>` | `PreferenceStore` + 19 holders (`PreferenceModule`) | E | area:02-preferences-system |
| `manga_db: Db`, `anime_db: Db` | `sqlDriverManga`, `sqlDriverAnime`, `Database`, `AnimeDatabase`, `MangaDatabaseHandler`, `AnimeDatabaseHandler` | L, W | areas 03, 04, 05, 06 |
| `json`, `xml`, `proto` | `Json`, `XML`, `ProtoBuf` | none (functions/consts) | section 6 of `p1-rust-design.md` |
| `chapter_cache: ChapterCache` | `ChapterCache(app, json)` | L | area:34-images-covers-cache |
| `manga_cover_cache`, `anime_cover_cache`, `anime_background_cache` | `MangaCoverCache`, `AnimeCoverCache`, `AnimeBackgroundCache` | L | area:34 |
| `network: NetworkHelper` | `NetworkHelper(app, NetworkPreferences)` | L, W | area:12-network-stack |
| `js: JavaScriptEngine` | `JavaScriptEngine(app)` (QuickJS) | L | area:12 |
| `manga_sources: Arc<MangaSourceManager>`, `anime_sources` | `MangaSourceManager`, `AnimeSourceManager` | L, W | area:17-source-app-layer |
| `manga_extensions`, `anime_extensions` | `MangaExtensionManager`, `AnimeExtensionManager` | L | areas 18, 19, 20 |
| `manga_download_provider`, `manga_downloads`, `manga_download_cache` and anime twins | `*DownloadProvider`, `*DownloadManager`, `*DownloadCache` | L; `*_downloads` W | areas 23, 24, 25 |
| `trackers: TrackerManager` | `TrackerManager(app)` | L | area:27-tracking-core |
| `delayed_manga_tracking`, `delayed_anime_tracking` | `DelayedMangaTrackingStore`, `DelayedAnimeTrackingStore` | L | area:27 |
| `image_saver: ImageSaver` | `ImageSaver(app)` | L | area:34 |
| `storage_folders` | `AndroidStorageFolderProvider(app)` | L | area:13-core-util-storage-archive-torrent |
| `storage: StorageManager` | `StorageManager(app, StoragePreferences)` | L | area:13 |
| `local_manga_fs`, `local_manga_cover`; `local_anime_fs`, `local_anime_background`, `local_anime_cover`, `local_anime_fetch_type`, `local_episode_thumbnail` | `LocalMangaSourceFileSystem`, `LocalMangaCoverManager`, `LocalAnimeSourceFileSystem`, `LocalAnimeBackgroundManager`, `LocalAnimeCoverManager`, `LocalAnimeFetchTypeManager`, `LocalEpisodeThumbnailManager` | L | area:14-source-local-and-metadata |
| `external_player: ExternalPlayer` | `ExternalIntents()` | L | area:45-player-core |
| `torrent_api`, `torrent_utils` | `TorrentServerApi`, `TorrentServerUtils` | L | area:13 |
| `images: ImagePipeline` | Coil `ImageLoader` built by `App.newImageLoader` | L (first image request) | area:34, configured here (T-01a-029) |
| domain use cases (`DomainModule`, `SYDomainModule`) | many `addFactory` | not fields | area:10 (plain functions over repositories) |

Interface sketch (module `kuro_core::app_context`):

```rust
pub struct AppContext {
    pub platform: Arc<dyn Platform>,
    pub info: &'static AppInfo,
    pub rt: tokio::runtime::Handle,
    pub settings: Arc<Settings>,
    manga_db: OnceLock<Db>,
    anime_db: OnceLock<Db>,
    network: OnceLock<Arc<NetworkHelper>>,
    manga_sources: OnceLock<Arc<MangaSourceManager>>,
    anime_sources: OnceLock<Arc<AnimeSourceManager>>,
    manga_downloads: OnceLock<Arc<MangaDownloadManager>>,
    anime_downloads: OnceLock<Arc<AnimeDownloadManager>>,
    // plus exactly one OnceLock per remaining L-field named in the table above
}

impl AppContext {
    pub fn new(platform: Arc<dyn Platform>, settings: Arc<Settings>, rt: Handle) -> Arc<Self> { /* E fields only */ }
    pub fn manga_db(&self) -> &Db { self.manga_db.get_or_init(|| Db::open(self.platform.paths().manga_db(), &MANGA_SCHEMA).expect("open manga db")) }
    pub fn network(&self) -> &Arc<NetworkHelper> { self.network.get_or_init(|| Arc::new(NetworkHelper::new(&self.settings.network))) }
    // every L-field has an accessor of the same shape; an accessor obtains its dependencies by calling the accessors of other fields
}
```
Circular collaborators: `MangaSourceManager` needs the download manager only at use time (Kotlin: `private val downloadManager: MangaDownloadManager by injectLazy()`). In Rust the source manager stores `Weak<AppContext>` or, preferably, a `OnceLock<Arc<dyn DownloadLookup>>` that `AppContext::manga_sources()` fills after both exist. Never call an accessor of a field from inside its own `get_or_init` closure (deadlock, `OnceLock` re-entrancy panics); the dependency graph must stay a DAG, checked by the unit test T-01a-003 ("every accessor can be called first, in any order, on a fresh context").

Where services need the Android `Application`/`Context` (for files, resources, system services), Rust passes `Arc<dyn Platform>` or the specific trait, never the whole context.

## 3. Startup sequence in Rust (replaces `App.onCreate` + `MainActivity.onCreate`)

`fn main()` in `src/main.rs` (existing file keeps `build_ui`, tabs, sidebar):

1. `cli::parse(std::env::args_os())` → `Cli { crash_report: Option<PathBuf>, open: Option<DeepLink>, files: Vec<PathBuf>, version: bool, help: bool }`. `--version` prints `Kurobayomi <VERSION_NAME> (<COMMIT_SHA>, <VERSION_CODE>, <BUILD_TIME>)` and exits 0 before touching GTK. If `crash_report` is set: `crash_window::run(path)` and return its exit code (no other step runs).
2. `let platform = LinuxPlatform::new()` (resolves XDG directories; creates `config`, `state`, `data`, `cache` directories with mode 0700 if missing; failure to create a directory is fatal: print to stderr, exit 1).
3. `logging::init(&platform.paths(), verbose_hint)` where `verbose_hint` is `cfg!(debug_assertions)` until settings are readable (step 5 re-evaluates). T-01a-005.
4. `crash::install(&platform.paths())` (panic hook). T-01a-006. Steps 3 and 4 precede everything else so that errors in later steps are reported.
5. `Settings::load(&platform.paths())` (area:02). If `verbose_logging` differs from the hint, call `logging::set_verbose(bool)` (tracing reload handle). A corrupt `settings.toml` is renamed to `settings.toml.bad-<unix_ms>` and defaults are used; a warning is logged (this is stricter than Aniyomi, whose SharedPreferences cannot be corrupt in practice).
6. Build the tokio runtime: `tokio::runtime::Builder::new_multi_thread().worker_threads(workers).enable_all().thread_name("kuro-rt").build()` with `workers = clamp(available_parallelism, 2, 8)`; keep the `Runtime` alive in `main`'s stack frame; pass `rt.handle().clone()`.
7. `AppContext::new(platform, settings, rt_handle)`.
8. Create `gtk::Application::builder().application_id(info.app_id).flags(HANDLES_COMMAND_LINE | (NON_UNIQUE if any KUROBAYOMI_SMOKE/OPEN/SHOT env var is set))`. Keep the existing dev-env behaviour.
9. `connect_startup` (runs once in the primary instance): `theme_mode::apply(settings.ui.theme_mode)` and subscribe to changes; `notify::register_categories(&ctx)`; `IncognitoIndicator::install(&ctx)`; `ProcessLifecycle::install(&ctx)`; initialise the GTK-side services (CSS provider, icon theme paths).
10. `connect_command_line` / `connect_activate`: on the first activation run `migrator::run_blocking(&ctx)` (section 5 of `p1-topic-migrator.md`), then `build_ui(app, &ctx)` (existing function extended with the context), then `warm_up(&ctx)`; on later activations present the existing window and dispatch the `DeepLink` carried by the command line.
11. `app.run_with_args(&[])` returns the exit code; `main` returns it. `drop(rt)` at the end with `shutdown_timeout(Duration::from_secs(2))` so in-flight blocking tasks do not hang the exit.

Step order differs from Kotlin in two deliberate places: logger and crash handler first (Kotlin installs logger at step 15 of `onCreate`), and migrations run before the first window exists (Kotlin runs them on `Migrator.scope` and `MainActivity` blocks in `onCreate` until finished, which is the same observable behaviour).

## 4. Warm-up (replaces the `ContextCompat.getMainExecutor(app).execute` block of `AppModule.kt:241-253`)

After the main window is presented, schedule on the GTK main loop with `glib::idle_add_local_once` one step per idle callback, in this order, logging `"warm-up <name> took <ms> ms"` at DEBUG:
1. `ctx.network()`
2. `ctx.manga_sources()`
3. `ctx.anime_sources()`
4. `ctx.manga_db()` (spawn on the blocking pool because opening runs schema creation/migration; the idle callback awaits the join handle through `glib::MainContext::spawn_local`)
5. `ctx.anime_db()` (same)
6. `ctx.manga_downloads()`
7. `ctx.anime_downloads()`
Why: the order and the set are Aniyomi's (`AppModule.kt:243-252`); doing them after the first frame keeps cold start fast, which is the stated reason in the source comment ("Asynchronously init expensive components for a faster cold start"). Difference: the first screen needs the database immediately (library), so `build_ui` must call `ctx.manga_db()` itself when it needs data; the warm-up only guarantees they are ready soon. If a DB open fails, show the crash window path: the error propagates as a panic from `expect`, which the hook reports (a corrupt database is fatal and unrecoverable without user action; the crash screen text contains the SQLite error and the DB path).

## 5. Threading model

* GTK main thread: all widgets, `PrefBinding` callbacks, `gio` actions. Equivalent of Android's main looper and `Dispatchers.Main`.
* tokio multi-thread runtime: network, parsing, image decode scheduling, downloads, update jobs. Equivalent of `Dispatchers.IO`/`Default`.
* Blocking pool (`spawn_blocking`): rusqlite calls, file system scans, archive reads. Equivalent of the IO dispatcher's blocking use.
* Bridge UI → async: `glib::MainContext::default().spawn_local(async move { let r = ctx.rt.spawn(job).await; update_widgets(r) })` (a tokio `JoinHandle` can be awaited from the glib executor; no extra crate).
* Bridge async → UI: a task running on tokio sends results through `async_channel::Sender` (or `tokio::sync::watch`) that a `spawn_local` task on the main context receives; or `glib::MainContext::default().invoke(closure)` for one-shot callbacks. Never touch a GTK object from a tokio worker; GTK-rs enforces this with `!Send` types, which is the intended safety net.
* Never call `Handle::block_on` on the GTK thread, with one exception: `migrator::run_blocking` at startup (matches Kotlin's `runBlocking` in `MainActivity.onCreate`).
* Concurrency limits carried over: image fetch 8, image decode 3 (section 11 of `p1-behavior.md`); each is a `tokio::sync::Semaphore` in `ImagePipeline`.

## 6. `IncognitoIndicator` (replaces the ongoing notification + BroadcastReceiver)

State source: `state.incognito_mode` preference (`PrefBinding<bool>`).
* On `true`: (a) `gio::SimpleAction "app.incognito-off"` becomes enabled (it is always registered; its handler sets `incognito_mode = false`); (b) the main window's status banner shows an "Incognito mode" strip with a "Disable incognito mode" button bound to that action (`gtk::Revealer`, 200 ms slide); (c) one `gio::Notification` id `"incognito_mode_channel:-701"` titled with `pref_incognito_mode`, body `notification_incognito_text`, priority LOW, default action `app.incognito-off` is sent via the `Notifier` trait.
* On `false`: hide the banner, withdraw notification `"incognito_mode_channel:-701"`.
* At startup the indicator reads the stored value once and applies it (so a persisted `true` shows the banner again, matching the Kotlin flow emitting the stored value at collection start).
* One-shot semantics are not needed: the action is idempotent.
* The status tint of `MainActivity` (`IncognitoModeBannerBackgroundColor`) becomes a CSS class `incognito` on the window; the colour token comes from area:49-theme-and-design-system.

## 7. `ProcessLifecycle`

```rust
pub enum LifecycleEvent { Started, Stopped }
pub struct ProcessLifecycle { /* gtk::Application windows' is-active notify + 700 ms debounce */ }
impl ProcessLifecycle { pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<LifecycleEvent>; }
```
Algorithm: on every toplevel `notify::is-active`, recompute `active = any window active`. Rising edge: cancel pending stop timer; if the process was stopped emit `Started`. Falling edge: start `glib::timeout_add_local_once(700 ms)`; when it fires and `active` is still false emit `Stopped`. Initial state: stopped; the first `is-active = true` emits `Started`. (Aniyomi: `ProcessLifecycleOwner` ON_START at first activity start; ON_STOP 700 ms after the last activity stopped.)

## 8. Plugging into the existing mockup (`src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/spec.rs`)

* `main()` today builds `gtk::Application` and calls `build_ui` on activate; the plan keeps `build_ui` and the helpers (`tabs`, `sidebar`) intact and changes only: (1) `main` gains steps 1-8 above; (2) `build_ui(app, ctx)` stores `ctx` in a thread-local `UI_CTX: RefCell<Option<Rc<UiContext>>>` inside `ui.rs` (the mockup already keeps `ui::nav_init(&nav, &win)` state in thread-locals, and so does `with_spec`), exposed as `ui::ctx()`, so screens that become real can read services without threading a parameter through every builder function; (3) dev env hooks `KUROBAYOMI_SMOKE`, `KUROBAYOMI_OPEN`, `KUROBAYOMI_SHEET`, `KUROBAYOMI_SIZE`, `KUROBAYOMI_SHOT`, `KUROBAYOMI_RAIL` stay unchanged (documented in a `--help` text), and `KUROBAYOMI_CRASH_TEST=1` is added to panic after the window shows (T-01a-048).
* `spec/*.json` stays the mockup's UI description; nothing in this area reads it. Real data replaces the hard-coded badge strings in `sidebar()` (`"updates" => "8"`, `"browse" => "1"`) when area:37/area:42 provide the counts through `ctx`.
* The crash window is a separate module (`crash_window.rs`) that reuses `ui::{column, heading, dim, scrolled, margins}` helpers from `src/ui.rs` and does not call `build_ui`.

## 9. Pitfalls
* `gtk::init` must run on the main thread before any widget; `Application::run` does it. The crash reporter mode must call `gtk::init()` itself (no `Application` needed, or an `Application` with a different id and `NON_UNIQUE`).
* A panic inside a GTK signal handler unwinds through C frames, which is undefined behaviour; gtk-rs aborts the process in that case. Therefore the panic hook must do all reporting work before returning (it runs before unwinding starts), and `panic = "abort"` is acceptable in release (smaller binary) provided the hook spawns the reporter first. Keep `panic = "unwind"` in dev so tests can use `catch_unwind`.
* Do not call `std::process::exit` from inside a tokio worker while holding the DB writer lock; WAL guarantees consistency but a lock held by a `spawn_blocking` thread is simply abandoned, which is fine.
* `OnceLock::get_or_init` closures must not call back into the same accessor (re-entrancy deadlock).
