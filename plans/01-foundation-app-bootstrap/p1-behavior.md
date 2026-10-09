# Area 01 — Behavior specification

OBSERVED = what the Kotlin does (with `path:line`). Proposal = what the Rust app should do. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Where a statement comes from a library that is not in the repository (AndroidX, Coil, kotlinx.coroutines), it is marked "library behaviour" and is not a claim about this repository.

Abbreviations: `App.kt` = `app/src/main/java/eu/kanade/tachiyomi/App.kt`; `AppModule.kt` = `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt`; `PreferenceModule.kt` = `app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt`; `GEH` = `app/src/main/java/eu/kanade/tachiyomi/crash/GlobalExceptionHandler.kt`.

## 1. Process model (OBSERVED)

* There are two OS processes in Aniyomi: the main process (package `xyz.jmir.tachiyomi.mi`, plus suffix `.dev`/`.debug`/`.benchmark` per build type) and the crash process. `CrashActivity` is declared in `app/src/main/AndroidManifest.xml:130-133` as `android:name=".crash.CrashActivity"`, `android:exported="false"`, `android:process=":error_handler"`. Because it runs in a separate process, Android instantiates a second `App` (the `Application` subclass) in that process; the `App.onCreate` code therefore runs in both processes. The only per-process guard in `App.onCreate` is the WebView data-directory suffix (`App.kt:93-97`), which exists precisely to avoid the "Using WebView from more than one process at once with the same data directory" crash.
* Proposal: Kurobayomi has one binary with two modes selected by argv: normal mode and crash-reporter mode (`kurobayomi --crash-report <file>`). The crash-reporter mode must not run the normal bootstrap (no database open, no migrations, no notifications, no single-instance registration). Full design in `p1-topic-crash-handling.md`.

## 2. `App.onCreate` sequence (OBSERVED `App.kt:82-165`)

The steps run on the Android main thread in exactly this order. The right-hand column is the Rust decision. "AC" = `AppContext` (`p1-topic-app-context-and-startup.md`).

| # | Kotlin step (line) | Behaviour | Rust decision |
|---|---|---|---|
| 1 | `super<Application>.onCreate()` (83) | Android framework init | none |
| 2 | `patchInjekt()` (84) | Replaces the Injekt singleton-factory with a thread-safe one. The patch library is `com.github.mihonapp:injekt:91edab2317` (`gradle/libs.versions.toml:43`); its README states it is "Patch for kohesive/injekt to make singleton factory thread safe" | Use `std::sync::OnceLock` / `LazyLock` for every lazily built service; thread safety comes for free. No code needed |
| 3 | `GlobalExceptionHandler.initialize(applicationContext, CrashActivity::class.java)` (86) | Installs the default uncaught-exception handler (section 12) | `kuro_core::crash::install()` must be the FIRST thing `main()` does, before GTK init, so a panic during bootstrap is reported |
| 4 | TLS provider (89-91): if `Build.VERSION.SDK_INT < Build.VERSION_CODES.Q` (29) then `Security.insertProviderAt(Conscrypt.newProvider(), 1)` | Gives Android 9 and older TLS 1.3 | Drop: `reqwest` uses `rustls` (TLS 1.2 and 1.3 everywhere). Task T-01a-027 records the decision |
| 5 | WebView suffix (94-97): if `SDK_INT >= P` (28): `val process = getProcessName(); if (packageName != process) WebView.setDataDirectorySuffix(process)` | Separate WebView data dir for the `:error_handler` process | Drop: the crash reporter never creates a web view and the main app is a single process. Task T-01a-028 |
| 6 | Modules (99-104): `Injekt.importModule(PreferenceModule(this))`, `AppModule(this)`, `DomainModule()`, `SYDomainModule()` (the last two wrapped by `// SY -->` / `// SY <--` comments) | Registers all singletons. Order matters only for module-level eager code: PreferenceModule registers factories only; AppModule additionally runs eager code (DB drivers are constructed immediately at registration, the main-executor warm-up is queued) | `AppContext::new()` builds the struct; ordering of eager work in section 13.3 |
| 7 | `setupNotificationChannels()` (106, 238-244) | `Notifications.createChannels(this)` inside try/catch; on `Exception` logs at ERROR `"Failed to modify notification channels"` with the throwable | Create the notification-category registry (section 9 of `p1-data-and-config.md`); failure is logged with the same message, never fatal |
| 8 | `ProcessLifecycleOwner.get().lifecycle.addObserver(this)` (108) | Registers `App` (a `DefaultLifecycleObserver`) for process-level start/stop | `ProcessLifecycle` helper (section 3) |
| 9 | `val scope = ProcessLifecycleOwner.get().lifecycleScope` (110) | Scope for the flows below; it is the main-thread (`Dispatchers.Main.immediate`) scope of the process lifecycle; it is cancelled never in practice (process lifetime) | GTK main-context tasks (`glib::MainContext::default().spawn_local`) owned by `AppContext` |
| 10 | Incognito notification flow (112-139) | Section 4 | `IncognitoIndicator` |
| 11 | `basePreferences.hardwareBitmapThreshold().let { if (!it.isSet()) it.set(GLUtil.DEVICE_TEXTURE_LIMIT) }` (141-143) | First launch only: stores the probed GPU texture size | Section 5 |
| 12 | `basePreferences.hardwareBitmapThreshold().changes().onEach { ImageUtil.hardwareBitmapThreshold = it }.launchIn(scope)` (145-147) | Pushes the preference into a global used by the reader's bitmap decision (`ImageUtil.kt:328-333`: `canUseHardwareBitmap` returns `maxOf(width, height) <= hardwareBitmapThreshold` unless `HARDWARE_BITMAP_UNSUPPORTED`) | Section 5 |
| 13 | `setAppCompatDelegateThemeMode(Injekt.get<UiPreferences>().themeMode().get())` (149) | One-shot read of the theme mode at launch; later changes are applied by the settings screen (area:47-settings-screens-general) calling the same function | Section 6 |
| 14 | `with(MangaWidgetManager(Injekt.get(), Injekt.get())) { init(lifecycleScope) }` and the Anime twin (152-158) | Home-screen widget refresh subscriptions | Drop (section 7) |
| 15 | `if (!LogcatLogger.isInstalled && networkPreferences.verboseLogging().get()) LogcatLogger.install(AndroidLogcatLogger(LogPriority.VERBOSE))` (160-162) | Logging stays silent unless the verbose preference is true | Section 8 |
| 16 | `initializeMigrator()` (164) | Runs preference/job migrations (section 10) | `Migrator` |

Note on ordering (OBSERVED, consequence): logging is installed at step 15, after steps 7 to 14; any `logcat { }` call before that is a no-op because the `logcat` library drops messages when no logger is installed (library behaviour). In particular the GlobalExceptionHandler's `logcat(priority = ERROR, throwable = exception)` produces output only when verbose logging was enabled AND the crash happened after step 15. Proposal: install the logger first (before step 3) and always write ERROR and above to the log file regardless of the verbose preference (section 8).

## 3. Process lifecycle observer (OBSERVED `App.kt:214-220`)

```
override fun onStart(owner: LifecycleOwner) { SecureActivityDelegate.onApplicationStart() }
override fun onStop(owner: LifecycleOwner)  { SecureActivityDelegate.onApplicationStopped() }
```

Library behaviour of `ProcessLifecycleOwner` (androidx.lifecycle:lifecycle-process, in `androidx.bundles.lifecycle` = lifecycle-common, lifecycle-process, lifecycle-runtimektx per `gradle/androidx.versions.toml:37-38`): `ON_START` fires when the first activity of the process starts; `ON_STOP` fires only after the last activity stopped and a 700 ms grace period elapsed without another activity starting (the constant `TIMEOUT_MS = 700` in the AndroidX class). The grace period swallows configuration-change recreations.

### 3.1 App-lock state machine (OBSERVED `app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt`)

State: `requireUnlock: Boolean` (static, initial `true`; "Always require unlock if app is killed"), preference values from `SecurityPreferences`: `useAuthenticator` (`use_biometric_lock`), `lockAppAfter` (`lock_app_after`, Int minutes: `-1` never, `0` always/immediately, positive N minutes), `lastAppClosed` (`__APP_STATE_last_app_closed`, Long epoch milliseconds; 0 when unset).

`onApplicationStopped()`:
1. if `useAuthenticator` is false: return.
2. if `AuthenticatorUtil.isAuthenticating` (a biometric prompt is on screen) is true: return (do nothing).
3. else if `requireUnlock` is true: return (the app closed while still locked; nothing to remember).
4. else if `lockAppAfter > 0`: `lastAppClosed.set(System.currentTimeMillis())`.
   (When `lockAppAfter` is `0` or `-1`, no timestamp is stored.)

`onApplicationStart()`:
1. if `useAuthenticator` is false: return.
2. if `!isAuthenticating && !requireUnlock`: `requireUnlock = when (lockAppAfter) { -1 -> false; 0 -> true; else -> lastAppClosed + lockAppAfter * 60_000 <= now }`.
3. always (when `useAuthenticator` is true): `lastAppClosed.delete()`.

`unlock()` sets `requireUnlock = false`. Per-activity side (`SecureActivityDelegateImpl`): on `onResume`, `setAppLock()`: if `useAuthenticator` is false return; if the device supports authentication (`activity.isAuthenticationSupported()`) then, when `requireUnlock` is true, start `UnlockActivity` with a zero-length transition (`overrideActivityTransition(OVERRIDE_TRANSITION_OPEN, 0, 0)` on API 34+, `overridePendingTransition(0, 0)` below); otherwise (authentication not supported) set `useAuthenticator` to false (the lock silently turns itself off).

Proposal (T-01a-019, T-01a-032): `ProcessLifecycle` emits `Started`/`Stopped` events consumed by an `AppLock` service owned by area:36-ui-shell-navigation-security-webview. Desktop mapping: "started" = at least one top-level window of the app is active (`gtk::Window::is_active()`); "stopped" = no window active for 700 ms (the 700 ms debounce kept so that a dialog opening, which briefly deactivates the main window, does not count as leaving). The state machine above is ported verbatim, with `now` from `SystemTime`. The biometric prompt is replaced by the unlock dialog described in area:36 (PAM or passphrase, per the `Biometrics/AppLock` row of `plans/addenda/requirements-cross-platform.md`). "Authentication not supported" maps to "no unlock method configured", and the same silent disable applies.

## 4. Incognito mode notification (OBSERVED `App.kt:112-139, 246-274`)

Source of truth: `BasePreferences.incognitoMode()` = boolean preference key `__APP_STATE_incognito_mode` (default false) — `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt`. It is an app-state key, therefore persisted across launches and never put in backups.

Collector (runs for the process lifetime): for every emitted value `enabled` of `incognitoMode().changes()` (the flow emits the stored value at collection start, then on every change):
* `enabled == true`:
  1. `disableIncognitoReceiver.register()` — if not already registered, `ContextCompat.registerReceiver(app, receiver, IntentFilter("tachi.action.DISABLE_INCOGNITO_MODE"), RECEIVER_NOT_EXPORTED)`, then `registered = true`.
  2. `notify(Notifications.ID_INCOGNITO_MODE, Notifications.CHANNEL_INCOGNITO_MODE) { ... }` with: content title = `MR.strings.pref_incognito_mode` ("Incognito mode"), content text = `MR.strings.notification_incognito_text` ("Disable incognito mode"), small icon `R.drawable.ic_glasses_24dp`, `setOngoing(true)`, content intent = `PendingIntent.getBroadcast(app, 0, Intent("tachi.action.DISABLE_INCOGNITO_MODE"), FLAG_ONE_SHOT or FLAG_IMMUTABLE)`.
     The channel is `incognito_mode_channel` with `IMPORTANCE_LOW`; the id is `-701`. `notify()` is a no-op on API 33+ when the `POST_NOTIFICATIONS` permission is not granted (`app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt:29-40`).
* `enabled == false`: `receiver.unregister()` (only if `registered`), then `cancelNotification(ID_INCOGNITO_MODE)`.

Receiver: `onReceive` ignores the intent contents and calls `basePreferences.incognitoMode().set(false)`. The preference change loops back through the collector, which unregisters and cancels. Tapping the notification therefore disables incognito and removes the notification. One-shot semantics: the PendingIntent is consumed on first use.

Consumers of the same preference elsewhere (context for the indicator): per-source incognito state (`GetMangaIncognitoState`, `GetAnimeIncognitoState`), the status-bar tint in `MainActivity` (`IncognitoModeBannerBackgroundColor`), secure-screen mode INCOGNITO (section 3.1 side: `SecureActivityDelegateImpl.setSecureScreen` combines `secureScreen()` and `incognitoMode()`: secure when mode is ALWAYS, or mode is INCOGNITO and incognito is on), and history writing. Those are other areas; this area only owns the notification.

Proposal (T-01a-020): see `p1-topic-app-context-and-startup.md` section 6. Linux has no ongoing notification that stays on screen reliably (freedesktop notifications expire; KDE keeps `resident`/`transient=false` ones in history only). The reliable equivalent is a persistent in-window banner plus an application action `app.incognito-off`. A `gio::Notification` with a default action `app.incognito-off` and button "Disable incognito mode" is sent once when incognito turns on and withdrawn when it turns off, as a best-effort mirror. No permission prompt exists on Linux.

## 5. Hardware bitmap threshold and `GLUtil` (OBSERVED)

`BasePreferences.hardwareBitmapThreshold()` = Int preference key `pref_hardware_bitmap_threshold`, default `GLUtil.SAFE_TEXTURE_LIMIT` = 2048 (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt`). `GLUtil` (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt`):

* `SAFE_TEXTURE_LIMIT = 2048`; private `MULTIPLIER = 1024`.
* `DEVICE_TEXTURE_LIMIT` (lazy): create an EGL10 display (`EGL_DEFAULT_DISPLAY`), `eglInitialize`, enumerate all configs, for each config read `EGL_MAX_PBUFFER_WIDTH` and keep the maximum, `eglTerminate`, result = `max(maximumTextureSize, SAFE_TEXTURE_LIMIT)`. The code comment says "opengl textures are always squared" so only the width is read.
* `CUSTOM_TEXTURE_LIMIT_OPTIONS` (lazy): `steps = DEVICE_TEXTURE_LIMIT / 1024` (integer division); list starts with `DEVICE_TEXTURE_LIMIT`, then for `step` from `steps` down to `2` inclusive: `value = step * 1024`; skip when `value >= DEVICE_TEXTURE_LIMIT`; otherwise append. Worked examples: device 16384 gives `[16384, 15360, 14336, 13312, 12288, 11264, 10240, 9216, 8192, 7168, 6144, 5120, 4096, 3072, 2048]` (15 entries); device 4096 gives `[4096, 3072, 2048]`; device 2048 gives `[2048]`; device 5000 gives `[5000, 4096, 3072, 2048]`.
* The list feeds the reader's advanced setting "hardware bitmap threshold" (area:47-settings-screens-general) and `ImageUtil.hardwareBitmapThreshold` decides whether a decoded bitmap may use a hardware (GPU-resident) bitmap: `canUseHardwareBitmap(width, height)` returns false when `HARDWARE_BITMAP_UNSUPPORTED`, else `maxOf(width, height) <= hardwareBitmapThreshold` (`core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:322-333`).

App start behaviour: if the key is not stored, store `DEVICE_TEXTURE_LIMIT` (so the stored value afterwards differs from the declared default 2048 on any device that reports more). Then every change (and the current value, because `changes()` emits first) is copied into `ImageUtil.hardwareBitmapThreshold`.

Proposal (T-01a-022, P2): GTK's GL and Vulkan renderers slice textures larger than the GPU limit on their own, so the threshold has no correctness role on Linux. Keep the preference key `pref_hardware_bitmap_threshold` (backup compatibility, area:21-backup-format-and-create) and keep the options algorithm byte-for-byte, with `device_texture_limit()` = environment override `KUROBAYOMI_TEXTURE_LIMIT` if set, else `max(probe, 2048)` where the probe is `GL_MAX_TEXTURE_SIZE` read once through a short-lived `gdk::GLContext` (fallback 16384 when no GL context can be created, for example under a software renderer without GL). The reader (area:43-reader-core) uses the value as the tile edge cap for very tall strips.

## 6. Theme mode (OBSERVED)

`UiPreferences.themeMode()` = enum preference key `pref_theme_mode_key`, default `ThemeMode.SYSTEM` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:20`), stored as the enum constant name. `ThemeMode` = `LIGHT`, `DARK`, `SYSTEM` (`app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt`). `setAppCompatDelegateThemeMode`: `LIGHT` → `AppCompatDelegate.MODE_NIGHT_NO`, `DARK` → `MODE_NIGHT_YES`, `SYSTEM` → `MODE_NIGHT_FOLLOW_SYSTEM`. App start applies it once.

Proposal (T-01a-021, P0): `theme_mode::apply(mode)` sets `gtk::Settings::default().set_gtk_application_prefer_dark_theme(...)` only for `LIGHT` (false) and `DARK` (true); for `SYSTEM` it must restore whatever GTK read from the desktop (store the initial value of the property at startup and write it back; never force light). The user's GTK theme and colour scheme stay in charge (user constraint recorded in memory: keep the current Tokyo Night look everywhere, never reset to stock). The module re-applies on preference change through `PrefBinding`. The wider theme system (accent colour, "app theme", AMOLED) belongs to area:49-theme-and-design-system.

## 7. Widget managers (OBSERVED `presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/{manga/MangaWidgetManager,anime/AnimeWidgetManager}.kt`)

Both classes are identical in structure. `init(scope)` builds `combine(getUpdates.subscribe(read=false|seen=false, after = DateLimit.toEpochMilli()), securityPreferences.useAuthenticator().changes()) { a, b -> a to b }` where `DateLimit = ZonedDateTime.now().minusMonths(3).toInstant()` (`BaseMangaUpdatesGridGlanceWidget.kt:159-160`, evaluated each time the getter is read; it is read once when `init` runs, so the window start is fixed at process start). `.distinctUntilChanged { old, new -> old.second == new.second && old.first.map { it.chapterId }.toSet() == new.first.map { it.chapterId }.toSet() }` (anime variant uses `episodeId`), then on each emission `MangaUpdatesGridGlanceWidget().updateAll(context)` and `MangaUpdatesGridCoverScreenGlanceWidget().updateAll(context)` inside try/catch that logs ERROR `"Failed to update widget"`; collected `flowOn(Dispatchers.Default)`, `launchIn(scope)`.

Decision (T-01a-024): Drop the Glance home-screen widgets (no equivalent surface on Linux desktops). Replacement kept: the unread-updates badge already present in the mockup sidebar (`src/main.rs` shows "8" on Updates), driven by the same query (unread updates created in the last 3 months). Optional replacement T-01a-025 (P2): launcher badge through the `com.canonical.Unity.LauncherEntry` D-Bus signal, which KDE Plasma's task manager honours. The user loses: tapping a cover on the Android home screen to open the entry; hidden-content behaviour of the locked widget (the widget shows a locked state while the app lock is on, `LockedAnimeWidget.kt`).

## 8. Logging (OBSERVED)

* Library: `com.squareup.logcat` (`libs.logcat`). Aniyomi wraps it as `tachiyomi.core.common.util.system.logcat`.
* `NetworkPreferences.verboseLogging()` = boolean key `verbose_logging`, default = `isDebugBuildType` passed in by `PreferenceModule` (`app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt`: `BuildConfig.BUILD_TYPE == "debug"`). So debug builds log verbosely by default; preview and release builds do not.
* Installation rule (App.kt:160-162): install `AndroidLogcatLogger(LogPriority.VERBOSE)` once if not installed and the preference is true at launch. Toggling the preference later does not install or remove the logger until next launch (the settings screen shows a restart hint; area:47-settings-screens-general). Coil's own logger: `if (verboseLogging) logger(DebugLogger())` evaluated when the image loader is built.
* Proposal (T-01a-005): `tracing` + `tracing-subscriber`. Always-on layer: `ERROR` and `WARN` to `$XDG_STATE_HOME/kurobayomi/logs/kurobayomi.log` (daily rotation, keep 7 files) so crash reports can attach the tail. Verbose layer: when `verbose_logging` is true or the environment variable `RUST_LOG` is set, additionally emit `TRACE`..`INFO` to stderr and the file. Filter default string: `warn,kurobayomi=info` normally, `kurobayomi=trace,kuro_core=trace,warn` when verbose.

## 9. Notification channels at startup (OBSERVED `app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt`, called from `App.kt:240`)

`createChannels(context)` (a) deletes nine deprecated channel ids (`downloader_channel`, `downloader_complete_channel`, `backup_restore_complete_channel`, `library_channel`, `library_progress_channel`, `updates_ext_channel`, `downloader_cache_renewal`, `crash_logs_channel`, `library_skipped_channel`), (b) creates four channel groups (`group_backup_restore`, `group_downloader`, `group_library`, `group_apk_updates`) named by `MR.strings.label_backup`, `MR.strings.download_notifier_downloader_title`, `MR.strings.label_library`, `MR.strings.label_recent_updates`, (c) creates 13 channels. The complete table (ids, importance, group, name key, badge, sound) is in `p1-data-and-config.md` section 9. Android channels let the user mute each class of notification from OS settings; freedesktop notifications have no channel concept.

Proposal (T-01a-023): a `NotificationCategory` enum in `kuro-core::notify` carries the same 13 ids, mapped to freedesktop `category` and `urgency` hints and to an app-level enable switch (default true; new setting, P2). Notification ids become string keys (`"{channel}:{id}"`) so "update in place" (same id re-posted) keeps working with `gio::Notification::withdraw`/`send` or the `replaces_id` field. The deprecated-channel deletion step is dropped (no legacy installs).

## 10. Migrator (OBSERVED; full detail in `p1-topic-migrator.md`)

Trigger (`App.kt:167-180`): read Int preference `__APP_STATE_last_version_code` (default 0) as `old`; `new = BuildConfig.VERSION_CODE` (132); log `"Migration from {old} to {new}"`; call `Migrator.initialize(old, new, migrations, onMigrationComplete = { logcat "Updating last version to {new}"; preference.set(new) })`. `MainActivity.onCreate` blocks on `Migrator.awaitAndRelease()` (`MainActivity.kt:166`, uses `runBlocking`) so no UI is composed before migrations finish; the Boolean result tells the activity "didMigration" (used to show the changelog / onboarding logic, area:36).

Strategy selection (`MigrationStrategyFactory.create(old, new)`):
* `old == 0` → `InitialMigrationStrategy`: run only migrations with `isAlways` (version == `-1f`).
* `old >= new` → `NoopMigrationStrategy(false)`: nothing runs, the completion listener is NOT called, result false.
* else → `VersionRangeMigrationStrategy(versions = (old + 1)..new)`: run migrations where `isAlways || it.version.toInt() in versions` (truncation of the Float).

`DefaultMigrationStrategy`: empty list → `CompletableDeferred(false)`; otherwise build the chain and, in a fire-and-forget `launch` on `Migrator.scope`, `if (chain.await()) migrationCompletedListener()`; returns the chain.

Chain (`MigrationJobFactory.create`): sort by `version` ascending (`ALWAYS = -1f` first), fold from `CompletableDeferred(true)`; each element is `async(start = UNDISPATCHED) { val prev = acc.await(); migration(ctx) || prev }`. Each migration runs after the previous finished (sequential), always executes (the `||` evaluates the migration call first), and the chain result is true if any migration returned true. In dry-run mode (`dryrun = true`) each element is `CompletableDeferred(true)` and nothing executes (only logs `"(Dry-run) Running migration: ..."`). The log line for real runs is `"Running migration: { name = <class simple name>, version = <float> }"`.

Registered list: 44 migrations in `app/src/main/java/mihon/core/migration/migrations/Migrations.kt` (versions listed in `p1-topic-migrator.md` section 4). Highest version is 130 (`PrefLangMigration`), current version code 132. Three have version `ALWAYS`: `SetupBackupCreateMigration`, `SetupAnimeLibraryUpdateMigration`, `SetupMangaLibraryUpdateMigration` (each schedules a periodic job and returns true).

Proposal (T-01a-012, T-01a-013): port the framework unchanged in semantics (blocking at startup, sequential chain, `old == 0` runs only always-migrations, completion listener writes `last_version_code`). Kurobayomi's own settings file has no legacy keys, so the registered list starts with the always-migrations that set up schedulers (area:26-library-update-jobs) and every legacy Aniyomi migration is NOT ported (their only input is an Android SharedPreferences file that a Linux install can never contain; Aniyomi backups are handled by the restore-time preference mapping in area:22-backup-restore). `RUST_VERSION_CODE` starts at 1 and is incremented whenever a migration is added.

## 11. Image loader configuration (OBSERVED `App.kt:182-212`)

`App` implements `SingletonImageLoader.Factory`; Coil 3 calls `newImageLoader(context)` the first time any image is requested. Configuration, in code order:
1. `callFactoryLazy = lazy { Injekt.get<NetworkHelper>().client }` (the OkHttp client is only built when an image is first fetched over the network).
2. Components, in this order: `OkHttpNetworkFetcherFactory(callFactoryLazy::value)`; decoder `TachiyomiImageDecoder.Factory()`; fetchers `BufferedSourceFetcher.Factory()`, `MangaCoverFetcher.MangaFactory(callFactoryLazy)`, `MangaCoverFetcher.MangaCoverFactory(callFactoryLazy)`, `AnimeImageFetcher.AnimeFactory(callFactoryLazy)`, `AnimeImageFetcher.AnimeCoverFactory(callFactoryLazy)`; keyers `AnimeKeyer()`, `MangaKeyer()`, `AnimeCoverKeyer()`, `MangaCoverKeyer()`.
3. `crossfade((300 * this@App.animatorDurationScale).toInt())` milliseconds. `animatorDurationScale` is the system animation scale (`Settings.Global.ANIMATOR_DURATION_SCALE`); 0 disables the fade.
4. `allowRgb565(DeviceUtil.isLowRamDevice(this))`: true when total RAM is below 3 GiB (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt:95-100`, `totalMem < 3L * 1024 * 1024 * 1024`; the comment says the system flag is "not necessarily trustworthy").
5. `if (verboseLogging) logger(DebugLogger())`.
6. `fetcherCoroutineContext(Dispatchers.IO.limitedParallelism(8))` and `decoderCoroutineContext(Dispatchers.IO.limitedParallelism(3))` with the comment "Coil spawns a new thread for every image load by default": at most 8 concurrent fetches and 3 concurrent decodes process-wide.

Proposal (T-01a-029): image pipeline (owned by area:34-images-covers-cache) is constructed by `AppContext` with `fetch_permits = 8`, `decode_permits = 3` (tokio `Semaphore`s), cross-fade 300 ms when `gtk::Settings::is_gtk_enable_animations()` is true and 0 otherwise, `low_ram = MemTotal < 3 GiB` read from `/proc/meminfo` (T-01a-030, P2; RGB565 itself is not supported by GDK textures, so the flag only lowers the in-memory image cache size, value defined in area:34).

## 12. Crash handling (OBSERVED; full design in `p1-topic-crash-handling.md`)

### 12.1 `GlobalExceptionHandler` (`GEH`)
* `initialize(applicationContext, activityToBeLaunched)`: reads `Thread.getDefaultUncaughtExceptionHandler()` (non-null cast; throws NPE if none, never the case on Android) as `defaultHandler`, creates the handler, calls `Thread.setDefaultUncaughtExceptionHandler`.
* `uncaughtException(thread, exception)`: (1) `logcat(priority = ERROR, throwable = exception)`; (2) `launchActivity(...)`; (3) `defaultHandler.uncaughtException(thread, exception)` (the Android runtime handler logs "FATAL EXCEPTION" and kills the process).
* `launchActivity`: `Intent(applicationContext, activity)`; extra key `"Throwable"` = `Json.encodeToString(ThrowableSerializer, exception)`; flags `FLAG_ACTIVITY_CLEAR_TOP | FLAG_ACTIVITY_NEW_TASK` then `FLAG_ACTIVITY_CLEAR_TASK`; `applicationContext.startActivity(intent)`.
* `ThrowableSerializer`: descriptor `PrimitiveSerialDescriptor("Throwable", PrimitiveKind.STRING)`; serialize = `value.stackTraceToString()` (the full text, including causes and "Caused by:" sections, as one JSON string); deserialize = `Throwable(message = decoder.decodeString())`. Consequence: the receiving side's `exception.toString()` is `"java.lang.Throwable: "` followed by the whole original stack trace text (Kotlin/JVM `Throwable.toString()` is `className + ": " + message`).
* `getThrowableFromIntent(intent)`: `Json.decodeFromString(ThrowableSerializer, intent.getStringExtra("Throwable")!!)`; any `Exception` (missing extra gives NPE, malformed JSON) is logged at ERROR `"Wasn't able to retrieve throwable from intent"` and returns null.
* Every thread's uncaught exception reaches the handler, including coroutine failures that propagate to the thread handler (for example `Migrator.scope` failures; section 10 and `p1-topic-migrator.md` section 6).

### 12.2 `CrashActivity`
`onCreate`: `super.onCreate`; `WindowCompat.setDecorFitsSystemWindows(window, false)` (edge-to-edge); `val exception = GlobalExceptionHandler.getThrowableFromIntent(intent)`; `setComposeContent { CrashScreen(exception, onRestartClick = { finishAffinity(); startActivity(Intent(this, MainActivity::class.java)) }) }`. Extends `BaseActivity` (theme and the secure-screen delegate, area:36).

### 12.3 `CrashScreen` (`app/src/main/java/eu/kanade/presentation/crash/CrashScreen.kt`)
`InfoScreen` with icon `Icons.Outlined.BugReport`; heading `MR.strings.crash_screen_title` = "Whoops!"; subtitle `MR.strings.crash_screen_description` formatted with `MR.strings.app_name` = "%s ran into an unexpected error. We suggest you share the crash logs in our support channel on Discord." (app name is the non-translatable "Aniyomi"); primary button text `MR.strings.pref_dump_crash_logs` = "Share crash logs" runs `scope.launch { CrashLogUtil(context).dumpLogs(exception) }`; secondary (outlined) button text `MR.strings.crash_screen_restart_application` = "Restart the application" runs `onRestartClick`. Body: a rounded (`MaterialTheme.shapes.small`) box filling the remaining space, background `colorScheme.surfaceVariant`, containing `Text(exception.toString())` in `onSurfaceVariant`, padding 8 dp (`MaterialTheme.padding.small`), with 8 dp vertical padding around the box. When `exception` is null the text is the four characters `null`. Layout details are in `p1-ui-parity.md`.

### 12.4 `CrashLogUtil.dumpLogs(exception)` (`app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt`)
Runs in `withNonCancellableContext` (cannot be cancelled when the screen goes away). Steps:
1. `file = context.createFileInCacheDir("aniyomi_crash_logs.txt")` = `File(externalCacheDir, name)`, deleted first if it exists, then `createNewFile()` (`ContextExtensions.kt:104-111`).
2. Append `getDebugInfo() + "\n\n"`.
3. Append manga-extension section (if not null) `+ "\n\n"`; then anime-extension section (if not null) `+ "\n\n"`.
4. If an exception was supplied append `exception.toString() + "\n\n"`.
5. Run `Runtime.getRuntime().exec("logcat *:E -d -v year -v zone -f <path>").waitFor()`: dump (`-d`) of the Android log at priority ERROR and above, with year and time-zone in timestamps, written by logcat itself to the same file.
6. `uri = file.getUriCompat(context)` (FileProvider URI); `context.startActivity(uri.toShareIntent(context, "text/plain"))` which builds an `ACTION_SEND` chooser titled `MR.strings.action_share` ("Share"), with `EXTRA_STREAM = uri`, MIME `text/plain`, `FLAG_GRANT_READ_URI_PERMISSION`.
7. Any `Throwable` in steps 1-6: `withUIContext { context.toast("Failed to get logs") }` (a hard-coded English string, not a resource).

`getDebugInfo()` template (each line is literal text followed by the value):
```
App ID: ${BuildConfig.APPLICATION_ID}
App version: ${BuildConfig.VERSION_NAME} (${BuildConfig.COMMIT_SHA}, ${BuildConfig.VERSION_CODE}, ${BuildConfig.BUILD_TIME})
Android version: ${Build.VERSION.RELEASE} (SDK ${Build.VERSION.SDK_INT}; build ${Build.DISPLAY})
Android build ID: ${Build.DISPLAY}
Device brand: ${Build.BRAND}
Device manufacturer: ${Build.MANUFACTURER}
Device name: ${Build.DEVICE} (${Build.PRODUCT})
Device model: ${Build.MODEL}
WebView: ${WebViewUtil.getVersion(context)}
Current time: ${OffsetDateTime.now(ZoneId.systemDefault())}
MPV version: 6764488
Libplacebo version: v7.349.0
FFmpeg version: n7.1
```
The three player versions are literal constants in the source; a code comment says to use `Utils.VERSIONS.mpv`, `.libPlacebo`, `.ffmpeg` from `aniyomi-mpv-lib` 1.17.n onwards. `WebViewUtil.getVersion`: label and version name of `WebView.getCurrentWebViewPackage()` joined by a space, or the literal `"how did you get here?"` when null.

Extension sections (manga and anime are identical except one word): `available = extensionManager.availableExtensionsFlow.value.associateBy { it.pkgName }`; for each installed extension (`installedExtensionsFlow.value`) sorted by `name` (natural `String` order): `hasUpdate = (available[pkgName]?.versionCode ?: 0) > installed.versionCode`; skip the extension unless `hasUpdate || isObsolete`; entry text is three lines: `- {name}`, `  Installed: {installed.versionName} / Available: {available?.versionName ?: "?"}`, and `  Orphaned: {isObsolete}` (manga) or `  Obsolete: {isObsolete}` (anime; the inconsistent label is in the source, `CrashLogUtil.kt` anime function). Result is null when no entries, otherwise `"Problematic extensions:"` followed by the entries joined by `"\n"`.

### 12.5 Proposal summary
Panic hook writes a crash-report file and spawns the reporter process, then terminates; the reporter shows the same screen; "Share crash logs" writes `kurobayomi_crash_logs.txt` (same sections, Linux debug info, tail of the log file instead of logcat), copies its text to the clipboard and reveals it in the file manager; "Restart" spawns the normal app and exits. Details, constants and failure handling: `p1-topic-crash-handling.md`.

## 13. Dependency injection (OBSERVED)

### 13.1 Mechanism
Injekt (kohesive) with the Mihon thread-safety patch. `addSingleton(x)` registers an instance; `addSingletonFactory { }` registers a lazily created singleton (created on first `get`, then cached; the patch makes creation thread safe); `get<T>()` / `Injekt.get<T>()` resolves by type and throws if missing; `Injekt.getInstanceOrNull(T::class.java)` returns null instead (used by `MigrationContext.get<T>()`); `by injectLazy()` is a delegate that resolves on first access. Types are the keys, so there is exactly one instance per type. Tests and previews use `InMemoryPreferenceStore` and fakes by constructing classes directly.

### 13.2 `PreferenceModule` (`PreferenceModule.kt:31-96`) registrations, in order
`PreferenceStore` = `AndroidPreferenceStore(app)` (default SharedPreferences of the app); `NetworkPreferences(preferenceStore = get(), verboseLogging = isDebugBuildType)`; `SourcePreferences(get())`; `SecurityPreferences(get())`; `LibraryPreferences(get())`; `ReaderPreferences(get())`; `PlayerPreferences(get())`; `GesturePreferences(get())`; `DecoderPreferences(get())`; `SubtitlePreferences(get())`; `AudioPreferences(get())`; `TorrentPreferences(get())`; `AdvancedPlayerPreferences(get())`; `TrackPreferences(get())`; `DownloadPreferences(get())`; `BackupPreferences(get())`; `StoragePreferences(folderProvider = get<AndroidStorageFolderProvider>(), preferenceStore = get())`; `UiPreferences(get())`; `BasePreferences(app, get())`. All are lazy factories; every holder is a thin set of accessor functions over `PreferenceStore`, so construction is cheap and has no side effects.

### 13.3 `AppModule` (`AppModule.kt:73-254`) registrations, in order
1. `addSingleton(app)` — the `Application` instance.
2. Eager (constructed at registration time, not lazily): `sqlDriverManga = AndroidSqliteDriver(schema = Database.Schema, context = app, name = "tachiyomi.db", factory, callback)` and `sqlDriverAnime = AndroidSqliteDriver(schema = AnimeDatabase.Schema, context = app, name = "tachiyomi.animedb", factory, callback)`. `factory` = `FrameworkSQLiteOpenHelperFactory()` when `BuildConfig.DEBUG && SDK_INT >= R` (so Android Studio's Database Inspector works) else `RequerySQLiteOpenHelperFactory()` (bundled, newer SQLite from `libs.bundles.sqlite`: sqlite-framework, sqlite-ktx, sqlite-android). `callback` = `AndroidSqliteDriver.Callback(Schema)` with `onOpen(db)`: call `super.onOpen(db)` then run `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, `PRAGMA synchronous = NORMAL`, each executed via `db.query("PRAGMA ...")`, `cursor.moveToFirst()`, `cursor.close()`. (Opening of the actual database file happens lazily on first use by the helper; constructing the driver does not open it.)
3. Lazy singletons, in order: `Database(driver = sqlDriverManga, historyAdapter = History.Adapter(last_readAdapter = DateColumnAdapter), mangasAdapter = Mangas.Adapter(genreAdapter = StringListColumnAdapter, update_strategyAdapter = MangaUpdateStrategyColumnAdapter))`; `AnimeDatabase(driver = sqlDriverAnime, animehistoryAdapter = Animehistory.Adapter(last_seenAdapter = DateColumnAdapter), animesAdapter = Animes.Adapter(genreAdapter = StringListColumnAdapter, update_strategyAdapter = AnimeUpdateStrategyColumnAdapter, fetch_typeAdapter = FetchTypeColumnAdapter, memoAdapter = MemoColumnAdapter), episodesAdapter = Episodes.Adapter(memoAdapter = MemoColumnAdapter))`; `MangaDatabaseHandler` = `AndroidMangaDatabaseHandler(get(), sqlDriverManga)`; `AnimeDatabaseHandler` = `AndroidAnimeDatabaseHandler(get(), sqlDriverAnime)`; `Json { ignoreUnknownKeys = true; explicitNulls = false }`; `XML { defaultPolicy { ignoreUnknownChildren() }; autoPolymorphic = true; xmlDeclMode = Charset; indent = 2; xmlVersion = XmlVersion.XML10 }`; `ProtoBuf` (the default `ProtoBuf` companion instance); `ChapterCache(app, get())` (needs Json); `MangaCoverCache(app)`; `AnimeCoverCache(app)`; `AnimeBackgroundCache(app)`; `NetworkHelper(app, get())` (needs NetworkPreferences); `JavaScriptEngine(app)`; `MangaSourceManager` = `AndroidMangaSourceManager(app, get(), get())`; `AnimeSourceManager` = `AndroidAnimeSourceManager(app, get(), get())` (parameters: extension manager and stub-source repository, resolved by type); `MangaExtensionManager(app)`; `AnimeExtensionManager(app)`; `MangaDownloadProvider(app)`; `MangaDownloadManager(app)`; `MangaDownloadCache(app)`; `AnimeDownloadProvider(app)`; `AnimeDownloadManager(app)`; `AnimeDownloadCache(app)`; `TrackerManager(app)`; `DelayedAnimeTrackingStore(app)`; `DelayedMangaTrackingStore(app)`; `ImageSaver(app)`; `AndroidStorageFolderProvider(app)`; `LocalMangaSourceFileSystem(get())`; `LocalMangaCoverManager(app, get())`; `LocalAnimeSourceFileSystem(get())`; `LocalAnimeBackgroundManager(app, get())`; `LocalAnimeCoverManager(app, get())`; `LocalAnimeFetchTypeManager(app, get())`; `LocalEpisodeThumbnailManager(app, get())`; `StorageManager(app, get())` (needs StoragePreferences); `ExternalIntents()`; `TorrentServerApi(get(), get())`; `TorrentServerUtils(get(), get())`.
4. Warm-up: `ContextCompat.getMainExecutor(app).execute { get<NetworkHelper>(); get<MangaSourceManager>(); get<AnimeSourceManager>(); get<Database>(); get<AnimeDatabase>(); get<MangaDownloadManager>(); get<AnimeDownloadManager>() }`. The comment says "Asynchronously init expensive components for a faster cold start". Mechanism: the block is posted to the main-thread handler, so it runs on the main thread right after `App.onCreate` returns and before the first activity draws; it is not parallel, it only defers the cost until after the application object is created. Constructing `Database`/`AnimeDatabase` forces the helper to open the file, which runs `onCreate`/`onUpgrade` (schema creation and `.sqm` migrations, areas 03 and 04) and the `onOpen` PRAGMAs. Constructing the source managers starts their background loaders (`CoroutineScope(Job() + Dispatchers.IO)`) which flip `isInitialized` to true when done.

Order consequence: the `Database` get runs after `NetworkHelper` and both source managers were created. The source managers' constructors use `injectLazy` for the download manager, so there is no cycle.

### 13.4 What is NOT in these modules (for the porter)
`DomainModule` and `SYDomainModule` (use cases, repositories) are in `app/src/main/java/eu/kanade/domain/` (area:10-app-domain-layer); the Rust equivalent is plain constructors on `AppContext` fields (no registry).

## 14. Preference helper types (OBSERVED)

### 14.1 `PreferenceMutableState<T>` (`app/src/main/java/eu/kanade/core/preference/PreferenceMutableState.kt`)
* Construction: internal `state = mutableStateOf(preference.get())`; `init` launches `preference.changes().onEach { state.value = it }.launchIn(scope)`.
* `value` getter returns `state.value` (the last value delivered by the flow, initial value from `get()`); the setter calls `preference.set(value)` and does NOT touch `state` directly: the UI value changes only when the change notification arrives (write-through, no optimistic update). `component1()` returns `state.value`; `component2()` returns `preference::set` (so destructuring `val (v, setV) = pref.asState(scope)` works).
* `fun <T> Preference<T>.asState(scope)` is the factory. Used by screen models across the app (17 call sites under `app/src/main/java/eu/kanade/tachiyomi/ui/**` and `app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt`).

### 14.2 `CheckboxState.TriState<T>.asToggleableState()`
`Exclude` → `ToggleableState.Indeterminate`; `Include` → `On`; `None` → `Off`. The model (out of scope, `core/common/.../preference/CheckboxState.kt`): `State.Checked/None` (two-state) and `TriState.Include/Exclude/None` with `next()`: `Exclude → None`, `Include → Exclude`, `None → Include`. A separate persisted enum `TriState` has `DISABLED`, `ENABLED_IS`, `ENABLED_NOT` with `next()`: `DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED`. Both cycles are "off → include → exclude → off".

Proposal (T-01a-036): GTK `CheckButton` only has active/inconsistent. Map `Include` → `active = true, inconsistent = false`; `Exclude` → `inconsistent = true` (display a minus); `None` → `active = false, inconsistent = false`. Click handler replaces the default toggle by `state = state.next()`.

## 15. Utility functions (OBSERVED `app/src/main/java/eu/kanade/core/util/`)

### 15.1 `insertSeparators(generator)` (`CollectionUtils.kt:8-21`)
Input list `L` of size `n`, `generator(before: T?, after: T?) -> R?` where `T : R`.
* `n == 0` → return empty list (generator never called).
* Otherwise iterate `i` from `-1` to `n-1` inclusive: `before = L.getOrNull(i)` (null for `i = -1`); if not null append it; `after = L.getOrNull(i + 1)` (null for `i = n-1`); `separator = generator(before, after)`; if not null append it.
Result layout: `[sep(null, L0)?, L0, sep(L0, L1)?, L1, ..., L(n-1), sep(L(n-1), null)?]`; generator is called exactly `n + 1` times in forward order. Typical use: date headers (`before` null → header for the first item; `after` null → optional footer).

### 15.2 `insertSeparatorsReversed(generator)` (`CollectionUtils.kt:26-39`)
`n == 0` → empty. Otherwise build `newList` iterating `i` from `n` down to `0`: `after = L.getOrNull(i)` (null at `i = n`), append if not null; `before = L.getOrNull(i - 1)` (null at `i = 0`); `separator = generator(before, after)`; append if not null. Return `newList.asReversed()`. Final layout is identical to the forward version; the difference is only the order in which `generator` is invoked (first call is `(L(n-1), null)`, last call is `(null, L0)`), which matters when the generator is stateful. The returned value is a reversed VIEW in Kotlin; a Rust implementation returns an owned `Vec`.

### 15.3 `HashSet<E>.addOrRemove(value, shouldAdd)`: `add` when true, else `remove`; returns Unit.
### 15.4 `fastFilterNot`, `fastPartition`, `fastCountNot`
`fastFilterNot(p)` = elements for which `p` is false (index loop via `fastFilter`). `fastPartition(p)` = `Pair(first = elements where p true, second = elements where p false)`, both in original order. `fastCountNot(p)` = number of elements where `p` is false (computed as `size` minus the count where `p` is true). The "fast" prefix means index-based iteration without iterator allocation, valid only for random-access lists the app created itself (KDoc warning). Rust: `iter().filter(|x| !p(x))`, `Iterator::partition`, `iter().filter(|x| !p(x)).count()`.

### 15.5 `ifMangaSourcesLoaded()` / `ifAnimeSourcesLoaded()`
A composable returning `remember { Injekt.get<MangaSourceManager>().isInitialized }.collectAsState().value` (the `StateFlow<Boolean>` declared at `domain/src/main/java/tachiyomi/domain/source/manga/service/MangaSourceManager.kt:12` and the anime twin). Callers (12 files, for example `BrowseAnimeSourceScreen.kt:83`, `GlobalAnimeSearchScreen.kt:27`, `AnimeSourcePreferencesScreen.kt:52`, `MigrateSeasonSelectScreen.kt:35`, `AnimeSourceSearchScreen.kt:50` and the manga counterparts) do `if (!ifXSourcesLoaded()) { LoadingScreen(); return }` at the top of `Content()`: a centered progress indicator until the source list is ready. The state is `false` from process start until the source manager finished its first load of installed extensions and stub sources.

### 15.6 `Observable<T>.asFlow()`
`callbackFlow`: subscribe an `Observer`: `onNext` → `trySend(t)` (drops if the channel is full/closed), `onError(e)` → `close(e)`, `onCompleted` → `close()`; `awaitClose { subscription.unsubscribe() }`. Dropped (T-01a-038).

## 16. `AppInfo` (OBSERVED `app/src/main/java/eu/kanade/tachiyomi/AppInfo.kt`)
Contract with extensions (kept by ProGuard, `proguard-rules.pro:63`): `getVersionCode()` = `BuildConfig.VERSION_CODE` (132; note in the KDoc: "differs between forks so logic should not rely on it", since extension-lib 1.3); `getVersionName()` = `BuildConfig.VERSION_NAME` ("0.18.2.1"; debug builds append `-<commit count>` via `versionNameSuffix`); `getSupportedImageMimeTypes()` = `ImageUtil.ImageType.entries.map { it.mime }` in declaration order: `image/avif`, `image/gif`, `image/heif`, `image/jpeg`, `image/jxl`, `image/png`, `image/webp` (since extension-lib 1.5). Proposal: T-01a-002.

## 17. `Constants` (OBSERVED `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt`)
Values and consumers are in `p1-data-and-config.md` section 7. Behavioural summary of the shortcut actions in `MainActivity.kt:462-500` (`handleIntentAction`). First, if the intent has Int extra `notificationId` > -1 the matching notification is dismissed through `NotificationReceiver.dismissNotification(context, notificationId, groupId = extra "groupId" default 0)`. Then by `intent.action`: `SHORTCUT_ANIMELIB` -> `HomeScreen.Tab.AnimeLib()`; `SHORTCUT_LIBRARY` -> `Tab.Library()`; `SHORTCUT_MANGA` -> read Long extra `manga`; if the extras are missing the intent is not handled (function returns false); otherwise `navigator.popUntilRoot()` and `Tab.Library(idToOpen)` (library tab with that manga opened); `SHORTCUT_ANIME` -> same with extra `anime` and `Tab.AnimeLib(idToOpen)`; `SHORTCUT_UPDATES` -> `Tab.Updates`; `SHORTCUT_HISTORY` -> `Tab.History`; `SHORTCUT_SOURCES` -> `Tab.Browse(false)` (sources list); `SHORTCUT_ANIMEEXTENSIONS` -> `Tab.Browse(true, true)` (extensions, anime); `SHORTCUT_EXTENSIONS` -> `Tab.Browse(true)` (extensions, manga); `SHORTCUT_DOWNLOADS` and `SHORTCUT_ANIME_DOWNLOADS` -> `popUntilRoot()` and `Tab.More(toDownloads = true)` (both land on the same More-tab download queue entry); `Intent.ACTION_SEARCH`, `Intent.ACTION_SEND` and `"com.google.android.gms.actions.SEARCH_ACTION"` -> global search with the query (area:36). Emitters: library-update notifiers (`SHORTCUT_UPDATES`), `NotificationHandler` (`SHORTCUT_DOWNLOADS`, `SHORTCUT_ANIME_DOWNLOADS`), `NotificationReceiver` (manga/anime open, extensions update), `ReaderActivity.kt:569` (open manga on back), widgets. `URL_HELP` is opened by the Help row in More (`MoreScreen.kt:172`) and by `onHelpClick` of browse/search/migration screens; `URL_HELP_UPCOMING` by the Upcoming screens' help button. `MAIN_ACTIVITY` is the class name used by widgets to build intents by reflection (`Class.forName`).

## 18. `SecurityPreferences` (OBSERVED)
Keys and defaults in `p1-data-and-config.md` section 5. Semantics: `useAuthenticator` master switch for the lock; `lockAppAfter` minutes, `-1` never, `0` always; `secureScreen` mode enum (name stored): `ALWAYS` (`MR.strings.lock_always` "Always"), `INCOGNITO` (`MR.strings.pref_incognito_mode` "Incognito mode", the default), `NEVER` (`MR.strings.lock_never` "Never"); `hideNotificationContent` hides notification text (consumers in library-update and download notifiers, area:26/area:35); `lastAppClosed` timestamp "will be set when there is a pending timed lock. Otherwise this pref should be deleted." (KDoc).

Secure-screen mode on Linux: Wayland and X11 offer no per-window "block screenshots / hide from recents" flag, so the mode is Dropped (T-01a-033); the setting row stays hidden. The user loses screenshot blocking and recent-apps thumbnail hiding; for an app lock the dialog still hides the content.

## 19. `DummyTracker` (OBSERVED)
Test double implementing `Tracker` (`app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt`, area:27-tracking-core): `id`, `name` required; defaults: `supportsReadingDates=false`, `supportsPrivateTracking=false`, `isLoggedIn=false`, `isLoggedInFlow=flowOf(false)`, `valLogoColor=Color.rgb(18, 25, 35)`, `valLogo=R.drawable.ic_tracker_anilist`, `valStatuses=(1L..6L).toList()`, `valCompletionStatus=2`, `valScoreList=(0..10).map(Int::toString)`, `val10PointScore=5.4`, empty search result lists. `client` getter is the Kotlin standard-library placeholder call with the message `Not yet implemented`, which throws `NotImplementedError` if accessed; `login` and `logout` and `saveCredentials` do nothing; `getUsername()` returns `"username"`; `getPassword()` returns `"passw0rd"`. Used by `TrackLogoIconPreviewProvider.kt`, `MangaTrackInfoDialogHomePreviewProvider.kt`, `AnimeTrackInfoDialogHomePreviewProvider.kt` (Compose previews only). Proposal T-01a-039: a `FakeTracker` with the same defaults for unit and screenshot tests; `client()` returns an error instead of panicking.

## 20. Build definition behaviour (OBSERVED `app/build.gradle.kts`)
Facts and the Cargo mapping are in `p1-topic-build-and-dependency-map.md`. Behaviours that affect runtime: `applicationId = "xyz.jmir.tachiyomi.mi"`; `versionCode = 132`; `versionName = "0.18.2.1"`; `BuildConfig.COMMIT_COUNT` (`git rev-list --count HEAD`), `COMMIT_SHA` (`git rev-parse --short HEAD`), `BUILD_TIME` (`yyyy-MM-dd'T'HH:mm:ss'Z'` UTC; release builds use the last commit time `git log -1 --format=%ct`, other types use the current time), `UPDATER_ENABLED` (true only when Gradle property `enable-updater` is passed); debug type: `applicationIdSuffix = ".dev"`, `versionNameSuffix = "-<commit count>"`, pseudo-locales on; release: minify and resource shrink on unless Gradle property `disable-code-shrink`; preview type inherits release with suffix `.debug`; benchmark type inherits release, profileable, suffix `.benchmark`, version suffix `-benchmark`, disabled unless the flavor pair `default`/`dev` is present; ABI splits for `armeabi-v7a`, `arm64-v8a`, `x86`, `x86_64` plus a universal APK.

## 21. Interactions with other areas
* area:02-preferences-system defines the real `PreferenceStore`/`Preference` contract that `PrefBinding`, `BasePreferences`, `SecurityPreferences` and `NetworkPreferences` use.
* area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations define table schemas and `user_version` migrations executed when `AppContext` opens the two databases (section 13.3 item 2 and 4).
* area:11-app-migrations lists the individual preference/job migrations registered with the migrator.
* area:12-network-stack owns `NetworkHelper`, `JavaScriptEngine` and `NetworkPreferences` use.
* area:20-extension-strategy-redesign owns how third-party sources are loaded; this area only supplies `AppInfo` and the host-API checklist (`p1-topic-build-and-dependency-map.md` section 5).
* area:34-images-covers-cache owns the image pipeline configured in section 11.
* area:35-data-misc-updater-notification-export owns the notification builders; this area owns channel registration at startup and the incognito notification.
* area:36-ui-shell-navigation-security-webview owns the app-lock UI, `MainActivity`, deep-link handling and splash behaviour.
* area:49-theme-and-design-system owns `InfoScreen` styling and the theme tokens used by the crash screen.
* area:52-util-app owns `CrashLogUtil` helpers such as file-in-cache and share intents; this area only specifies the crash-specific behaviour.
* area:53-platform-manifest-intents-workers owns `AndroidManifest.xml`, `shortcuts.xml` and the worker scheduling that replaces `Setup*Migration` jobs.
