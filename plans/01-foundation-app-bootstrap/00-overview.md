# Area 01 — Foundation: app bootstrap, DI, crash handling, app-level plumbing (overview)

Area id: `01-foundation-app-bootstrap`. Source scope: 19 Kotlin/Gradle/ProGuard files, 1601 LOC, as listed by `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 01-foundation-app-bootstrap 1`. All repo-relative paths are relative to the Aniyomi clone `/home/blackbox/aniyomi-src`.

## 1. Purpose and scope

**What Aniyomi does here.** The area is the process glue of the Android app:

* `App.onCreate` (`app/src/main/java/eu/kanade/tachiyomi/App.kt:82-165`) installs the global crash handler, registers the Injekt modules, sets up notification channels, observes process foreground/background, shows the incognito notification, applies the theme mode, starts the widget managers, and kicks off the startup migrator.
* `AppModule` and `PreferenceModule` (`app/src/main/java/eu/kanade/tachiyomi/di/`) are the service locator: two SQLite databases, source and extension managers, download managers, tracker manager, caches, network helper, JS engine, storage and local-source managers.
* The crash path (`GlobalExceptionHandler`, `CrashActivity`, `CrashScreen`, `CrashLogUtil`) turns an uncaught throwable into a separate "Whoops!" process with a shareable log file.
* Small plumbing: the app lock (biometric gate, `SecurityPreferences`), the incognito banner/notification, the source-ready gate, list helpers, the tri-state checkbox model, the image loader configuration, the build identity (`AppInfo`, `BuildConfig`), `Constants` (help URLs and intent names).
* Build definition: `app/build.gradle.kts` (version code 132, version name `0.18.2.1`, build types, ABI splits, dependency list) and the two ProGuard files.

**Why it matters for a usable Kurobayomi.** Every other area assumes a context object that yields the database handles, the preference store, the source managers and the download managers, and assumes that the startup migration has run before the first screen. Without this area nothing starts in a defined order, a panic leaves no report, and the preference handles that GTK widgets bind to do not exist. The P0 tasks of this area are the spine that lets the library, reader, player and source screens load.

**Scope boundaries.** This area owns the bootstrap order, the `AppContext` (DI replacement), the crash reporter, the migrator framework (not the individual migrations), the build/workspace layout, and the platform-neutral helpers. It does not own the preference storage (area:02), the schema (areas 03, 04), the individual migrations (area:11), the notification centre (area:35), the window chrome and the security delegate (area:36), or the release pipeline (area:54).

**Port decisions in one line.** Injekt becomes one plain `AppContext` struct with `OnceLock` fields. The Android Application lifecycle becomes an explicit `main()` sequence plus a GTK `connect_startup`/`connect_activate` pair. Crash reporting becomes a `std::panic::set_hook` that writes a report file and spawns the same binary in `--crash-report` mode. The migrator keeps its sequential-chain semantics. Widgets, RxJava, R8/ProGuard, Conscrypt, WebView package spoofing and FLAG_SECURE are dropped with reasons (see section 4).

## 2. Module map

### 2.1 Kotlin packages to responsibilities (OBSERVED)

| Package / path (repo-relative) | Responsibility | Files in scope |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/` | Application class, `AppInfo` | `App.kt`, `AppInfo.kt` |
| `app/src/main/java/eu/kanade/tachiyomi/di/` | Injekt modules: singletons and eager warm-up | `AppModule.kt`, `PreferenceModule.kt` |
| `app/src/main/java/eu/kanade/tachiyomi/crash/` | Uncaught-exception handler and crash activity | `GlobalExceptionHandler.kt`, `CrashActivity.kt` |
| `app/src/main/java/eu/kanade/presentation/crash/` | Crash screen (Compose) | `CrashScreen.kt` |
| `app/src/main/java/eu/kanade/core/util/` | Source-loaded gates, list helpers, Rx bridge | `AnimeSourceUtil.kt`, `MangaSourceUtil.kt`, `CollectionUtils.kt`, `RxJavaExtensions.kt` |
| `app/src/main/java/eu/kanade/core/preference/` | Compose state mirror of a preference, tri-state mapping | `PreferenceMutableState.kt`, `CheckboxState.kt` |
| `app/src/main/java/eu/kanade/test/` | Preview double of a tracker | `DummyTracker.kt` |
| `core/common/src/main/java/eu/kanade/tachiyomi/core/` | `Constants` (URLs, intent names) and `security/SecurityPreferences.kt` | `Constants.kt`, `SecurityPreferences.kt` |
| `app/src/main/java/mihon/core/migration/` (driven by this area, owned by area:11 for migrations) | Migrator framework | `Migrator`, `MigrationStrategy`, `Migration`, `MigrationJobFactory`, `MigrationContext`, `MigrationCompletedListener` (see `p1-topic-migrator.md`) |
| `app/build.gradle.kts`, `app/proguard-*.{txt,pro}` | Build definition, R8 rules | build files |

### 2.2 Proposed Rust workspace (PROPOSAL, from `p1-rust-design.md` section 1)

```
kurobayomi/                       workspace root + GUI package "kurobayomi" (existing mockup, src/*.rs)
  src/main.rs                     main(): cli -> platform -> logging -> crash hook -> settings -> runtime -> AppContext -> gtk::Application
  src/cli.rs                      NEW Cli { crash_report, open: Option<DeepLink>, files, version, help }
  src/crash_window.rs             NEW reporter-mode window (no build_ui call)
  src/incognito_indicator.rs      NEW banner + notification
  src/theme_mode.rs               NEW LIGHT/DARK/SYSTEM -> adw-free style manager
  src/prefs_bind.rs               NEW PrefBinding<T>, bind()
  src/widgets/tri_check.rs        NEW tri-state check button
  src/lifecycle.rs                NEW ProcessLifecycle (700 ms debounce)
  crates/kuro-core/               no GTK, no OS-specific code
    build.rs                      git info -> env
    src/app_info.rs               AppInfo, build constants
    src/app_context.rs            AppContext + OnceLock accessors
    src/bootstrap.rs              run_startup(): logging, crash, settings, runtime, context
    src/crash.rs                  hook, report file, CrashContext, contain()
    src/crash_dump.rs             debug info, extension blocks, log tail, dump writer
    src/logging.rs                tracing setup
    src/migrate/{mod,list}.rs     Version, Migration, Strategy, select, plan, run, run_blocking; MIGRATIONS static
    src/db.rs                     Db::open, PRAGMAs, user_version hook
    src/db_types.rs               UnixMillis, StringList, UpdateStrategy, FetchType, Memo
    src/notify.rs                 NotificationCategory table, ids
    src/links.rs                  URL_HELP, URL_HELP_UPCOMING
    src/deep_link.rs              DeepLink enum + parse/format
    src/util/collections.rs       insert_separators, insert_separators_reversed
    src/sources_gate.rs           SourceGate (is_initialized watch)
  crates/kuro-platform/           the only crate with cfg(target_os), D-Bus, libc-level calls
    src/lib.rs                    traits Paths, SecretStore, Notifier, MediaControls, WebViewHost, Opener, IdleInhibitor,
                                  SingleInstance, SchedulerBackend, FileAssociations, Autostart, ExternalPlayer, AppLock; spawn_detached
    src/linux/*.rs                implementations
  crates/kuro-testkit/            in-memory settings, temp-dir Paths, FakeTracker, fixtures
```

Other crates (`kuro-data`, `kuro-domain`, `kuro-source`, `kuro-source-local`, `kuro-i18n`) are created by their areas and depend on `kuro-core`. None depends on the GTK crate.

### 2.3 Crate choices owned by this area (from `p1-rust-design.md` section 2)

| Crate | Where | Replaces |
|---|---|---|
| `tokio` (`rt-multi-thread`, `sync`, `time`, `macros`) | kuro-core | `Dispatchers.IO/Default` |
| `tracing`, `tracing-subscriber` (`env-filter`, `fmt`, `reload`), `tracing-appender` | kuro-core | `logcat`, daily file rotation |
| `anyhow`, `thiserror` | all | Kotlin exceptions at app edges and library error enums |
| `rusqlite` (`bundled`) | kuro-core::db | SQLDelight/Android SQLite drivers |
| `serde`, `serde_json`, `toml` | kuro-core | Json defaults, settings files |
| `time` (`formatting`, `local-offset`) | kuro-core | `OffsetDateTime.now(ZoneId.systemDefault())` |
| `directories` | kuro-platform | Android `Context` directories behind `Paths` |
| `rustix` (`system`, `fs`) | kuro-platform | `uname`, `/proc` reads without `libc` unsafe |
| `async-channel` | GUI crate | UI mailbox for tokio results |
| `futures` | kuro-core | `BoxFuture` for migrations |
| `gtk4` (feature `v4_16`), `gio`, `glib` | GUI crate | Compose UI, notifications, actions |

No `lazy_static` or `once_cell`: `std::sync::{OnceLock, LazyLock}` and `std::cell::{OnceCell, LazyCell}` cover all needs (stable since Rust 1.80).

## 3. Reading guide

All files are in this folder. Read in the order given for a first pass.

1. [p1-inventory.md](p1-inventory.md): one row per in-scope file with LOC, every top-level declaration, port decision, target module, priority and task ids.
2. [p1-topic-app-context-and-startup.md](p1-topic-app-context-and-startup.md): the `AppContext` struct (replaces Injekt), the startup sequence, warm-up, threading model, incognito indicator, process lifecycle.
3. [p1-topic-crash-handling.md](p1-topic-crash-handling.md): panic hook algorithm, crash report file format, reporter mode, "share crash logs", restart, edge cases, test hook.
4. [p1-topic-migrator.md](p1-topic-migrator.md): the Aniyomi migrator contract and algorithm, the registration list, the Rust design of `select`, `plan`, `run`, `run_blocking`, dry run.
5. [p1-topic-build-and-dependency-map.md](p1-topic-build-and-dependency-map.md): Cargo workspace and profiles, every Gradle dependency mapped to a Rust answer, native libraries and their Linux counterparts, Kotlin opt-ins, ProGuard "host API surface" checklist for area:20.
6. [p1-behavior.md](p1-behavior.md): OBSERVED behaviour of every scope file (startup sequence, lifecycle, app-lock state machine, incognito, hardware bitmap threshold, theme, widgets, logging, notifications, migrator, image loader, crash, DI registrations, preference helper types, utilities, AppInfo, Constants, SecurityPreferences, DummyTracker, build).
7. [p1-data-and-config.md](p1-data-and-config.md): build identity constants, dependency declarations, preference store semantics, preference holders, preference keys, enums and stored values, `Constants` values, file and folder layout (Android and Linux), crash file format, notification channels, databases touched, endpoints, user-visible strings, `AppInfo` payload.
8. [p1-rust-design.md](p1-rust-design.md): workspace layout, crate table, key Rust signatures (`AppInfo`, `PrefBinding`, `TriState`, `SourceGate`, collections, `Db::open`, `NotificationCategory`, `DeepLink`), error policy, Android-to-Linux API mapping for the scope, serialization singletons, pitfalls.
9. [p1-ui-parity.md](p1-ui-parity.md): what the crash screen and incognito notification look like in Aniyomi and the Kurobayomi window design; what the existing mockup lacks.
10. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases and failure modes by group, the upstream migrator test to port, new unit tests per module, fixtures, release manual checklist.
11. [p1-tasks.md](p1-tasks.md): 48 tasks (`T-01a-001` to `T-01a-048`) in the required format; lint status `ok`.
12. [00-overview.md](00-overview.md): this document.

## 4. Key design decisions and open questions

Decisions already made in the part docs (recommendation: keep unless the user objects).

| # | Decision | Where | Recommendation |
|---|---|---|---|
| D1 | No DI container: one `AppContext` struct, `OnceLock` per lazy service | `p1-topic-app-context-and-startup.md` s1-2 | Keep. Test: every accessor callable first on a fresh context (T-01a-003). |
| D2 | Crash reporter is the same binary in `--crash-report <file>` mode | `p1-topic-crash-handling.md` s5 | Keep. One artifact to install. |
| D3 | Panic hook does all reporting work before returning; release uses `panic = "abort"` after the hook spawns the reporter, dev/tests keep `unwind` | `p1-topic-app-context-and-startup.md` s9 | Keep. Panics inside GTK callbacks are UB otherwise. |
| D4 | Migrations run before the first window, blocking the GTK thread once (`run_blocking`) | `p1-topic-migrator.md` s6 | Keep. Same observable behaviour as `MainActivity` `awaitAndRelease`. |
| D5 | Warm-up after first frame, one service per idle callback | `p1-topic-app-context-and-startup.md` s4 | Keep. Library still opens the DB synchronously when it needs data. |
| D6 | Settings file is TOML (`settings.toml`, `state.toml`), corrupt file renamed to `settings.toml.bad-<unix_ms>` | `p1-data-and-config.md` s5.6 | Keep. Stricter than SharedPreferences. |
| D7 | Crash logs: copy to clipboard and reveal the file, no share sheet | `p1-topic-crash-handling.md` s1 | Keep. There is no Linux share sheet. |
| D8 | Incognito: window banner plus one notification `incognito_mode_channel:-701`, idempotent disable action | `p1-topic-app-context-and-startup.md` s6 | Keep. |
| D9 | Widgets, Conscrypt, WebView package spoof, WebView data suffix, RxJava bridge, R8 rules, FLAG_SECURE, extension-installer preference: dropped | section 5 below | Keep. |

Open questions that need the user's call.

| # | Question | Options | Recommendation |
|---|---|---|---|
| Q1 | Does Kurobayomi ever import Aniyomi's own on-device data (SharedPreferences, SQLite files) directly, or only through backup files? | (a) backup file only (areas 21, 22), (b) direct import of `aniyomi.db` / `*.xml` prefs | (a). Direct import needs the Android layout and is not in this area. Confirm before area:22 is scheduled. |
| Q2 | App lock: keep it, and with which unlock? Aniyomi uses `BiometricPrompt` (`SecureActivityDelegate.kt`). | (a) drop the lock, (b) PIN stored with Argon2 in `kuro-platform::SecretStore` (libsecret or keyring), (c) polkit prompt | (b) if the user wants a lock at all. The state machine (T-01a-032) is platform-neutral, so it can be built before the choice. |
| Q3 | Verbose logging default. Aniyomi defaults to `BuildConfig.DEBUG`. | (a) same, (b) always off in release, `RUST_LOG` overrides | (b). The file log stays at INFO. |
| Q4 | Crash report contents: include the extension list (`manga_extensions`, `anime_extensions` sections) and the log tail? | (a) yes, (b) only names, no URLs | (b). Repository URLs and extension package names only. No user library data. |
| Q5 | Sidebar badge counts (`updates`, `browse`) are hard-coded in `src/ui.rs` today. Who fills them? | owned by area:37 and area:42 through `ctx` | Confirm with area:42 lead; this area only provides the mechanism. |
| Q6 | Window-level incognito banner or only notification? | (a) both (recommended), (b) notification only | (a). |
| Q7 | Should the launcher badge (T-01a-025, P2) exist? | KDE and GNOME dock badges through `org.freedesktop.Notifications` or `Unity` launcher API | Keep it P2 and optional. |

## 5. Dropped Aniyomi behaviour (P3) and what the user loses

| Task | Dropped item | Reason | What the user loses |
|---|---|---|---|
| T-01a-024 | Home-screen widget managers (`MangaWidgetManager`, `AnimeWidgetManager`) | No Linux equivalent that stays current without an app widget host; the sidebar badges give the same count | Desktop widgets. Badge counts remain in the sidebar. |
| T-01a-026 | WebView `getPackageName` override | Android-only workaround for WebView user-agent detection | Nothing. |
| T-01a-027 | Conscrypt TLS provider insertion | Android TLS stack hack; `rustls` (or the system TLS through reqwest) is used | Nothing. |
| T-01a-028 | WebView data-directory suffix | Android multi-process WebView issue | Nothing. |
| T-01a-033 | `FLAG_SECURE` secure-screen mode | Linux compositors have no per-window capture block | Screenshots of the app are not blocked. Notification privacy (T-01a-034) remains. |
| T-01a-038 | RxJava 1 `Observable.asFlow` bridge | No RxJava in Rust; legacy extension APIs become async (areas 15, 16) | Nothing user-visible. |
| T-01a-041 | R8/ProGuard keep rules | JVM shrinker. Release uses `strip`, LTO and the linker dead-code elimination | Nothing. The host-API surface checklist is kept and handed to area:20. |
| T-01a-044 | Extension installer preference (`ExtensionInstallerPreference`) | Installation mechanism is area:20's redesign | Choice of the Android package installer. |
| T-01a-046 | Picture-in-Picture capability flag | Mpv window is a normal toplevel; PiP is a compositor feature | Android PiP; the user can still float the window (compositor-specific). |

## 6. Cross-area dependencies

### 6.1 This area needs from others

| Area | What is needed | Tasks that depend on it |
|---|---|---|
| area:02-preferences-system | `Settings`, `Pref<T>`, `PreferenceStore` semantics, the 19 holders, `TriState` model, `changes()` stream | T-01a-003, T-01a-010, T-01a-011, T-01a-021, T-01a-034, T-01a-036, T-01a-045, T-01a-047 |
| area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations | Schema SQL and `user_version` numbers used by `Db::open` | T-01a-014 |
| area:05-data-repos-entries-items | Decision on pool type (`deadpool-sqlite` or hand-written) | T-01a-014 (indirect) |
| area:13-core-util-storage-archive-torrent | `Paths` storage layout and storage folder provider | T-01a-006 |
| area:17-source-app-layer | `SourceManager::is_initialized` watch | T-01a-037 |
| area:34-images-covers-cache | Image pipeline settings, cover cache types | T-01a-029 |
| area:35-data-misc-updater-notification-export | Notification categories and `Notifier` mapping | T-01a-023 |
| area:36-ui-shell-navigation-security-webview | Window chrome, `SecureActivityDelegate` behaviour, deep-link dispatch | T-01a-020, T-01a-032, T-01a-042 |
| area:43-reader-core | Texture-limit setting consumer | T-01a-022 |
| area:46-player-controls-ui | PiP decision | T-01a-046 |
| area:50-android-widgets | Widget drop decision | T-01a-024 |
| area:52-util-app | `CrashLogUtil` consumer behaviour | T-01a-008 |
| area:53-platform-manifest-intents-workers | Manifest shortcuts and launcher badge | T-01a-025 |
| area:20-extension-strategy-redesign | Host-API surface checklist and installer decision | T-01a-041, T-01a-044 |
| area:27-tracking-core | `Tracker` trait for `FakeTracker` | T-01a-039 |

### 6.2 Other areas need from this area

| Area | What they need | Provided by |
|---|---|---|
| area:02 | `kuro-core` crate and `kuro-testkit::TempPaths` for preference tests | T-01a-001, T-01a-045 |
| area:03, 04, 05, 06 | `AppContext::manga_db()` / `anime_db()`, `Db::open` with PRAGMAs, column adapters | T-01a-014, T-01a-015, T-01a-017 |
| area:10, 11 | Migrator framework and `MigrationContext` | T-01a-012, T-01a-013 |
| area:12-network-stack | `NetworkPreferences` holder and `AppContext::network()` accessor | T-01a-011, T-01a-017 |
| area:17, 18, 19 | `AppContext` accessors for source and extension managers; `AppInfo` for extension host queries | T-01a-002, T-01a-017 |
| area:23, 24, 25 | Download manager accessors and the warm-up contract | T-01a-017, T-01a-018 |
| area:27 | `AppContext::trackers()` accessor | T-01a-017 |
| area:34 | `ImagePipeline` concurrency limits (fetch 8, decode 3), low-RAM detection | T-01a-029, T-01a-030 |
| area:36 | `ProcessLifecycle` events, incognito indicator, app lock state machine | T-01a-019, T-01a-020, T-01a-032 |
| area:37, 38, 39, 40, 41, 42 | Theme mode application (`theme_mode::apply`), source-loaded gate, `DeepLink` and list helpers | T-01a-021, T-01a-035, T-01a-037 |
| area:49-theme-and-design-system | `theme_mode::apply` target (CSS class `incognito`, colour tokens) | T-01a-020, T-01a-021 |
| area:51-i18n | Message keys used by this area (`pref_incognito_mode`, `notification_incognito_text`, crash strings) | T-01a-020, T-01a-007 |
| area:54-build-release-distribution-docs | Cargo profiles, `build.rs` build identity, `--version` output | T-01a-002, T-01a-040 |

## 7. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, rows of `TASKS.csv` whose `file` starts with `01-foundation-app-bootstrap/` (48 rows). Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 01-foundation-app-bootstrap/p1-tasks.md` prints `ok`. All `area:` tokens resolve to existing ids.

### 7.1 Counts

| Priority | S | M | L | Total |
|---|---|---|---|---|
| P0 | 8 | 10 | 3 | 21 |
| P1 | 5 | 6 | 0 | 11 |
| P2 | 7 | 0 | 0 | 7 |
| P3 | 9 | 0 | 0 | 9 |
| **All** | **29** | **16** | **3** | **48** |

### 7.2 P0 critical path (in order)

Dependency-weighted longest chain (S=1, M=2, L=3) across P0 tasks:

1. `T-01a-001` Cargo workspace (M) → `T-01a-009` tokio runtime and GTK bridge (M) → `T-01a-003` AppContext (L) → `T-01a-012` migrator framework (M) → `T-01a-004` `main()` startup order (M) → `T-01a-042` single instance and deep link (P1, M). Weight 11 to `T-01a-004`, 13 to `T-01a-042`.
2. `T-01a-001` → `T-01a-005` logging (M) → `T-01a-006` panic hook and reporter spawn (L) → `T-01a-007` crash window (M). Weight 9. This chain must work before any other panic can be seen.
3. `T-01a-003` → `T-01a-014` database open with PRAGMAs (L; blocked by area:03 and area:04 schemas) → `T-01a-015` column adapters (S) and `T-01a-018` warm-up (S). Weight 11. This is the gate for every screen that reads data.
4. `T-01a-011` bootstrap preference groups (M, blocked by area:02) → `T-01a-047` settings load and recovery (S) → `T-01a-013` migration list (S). Needed before `T-01a-004` can run for real.

Parallel P0 work that is not on the critical path: `T-01a-002` build identity, `T-01a-010` PrefBinding, `T-01a-021` theme mode, `T-01a-029` image pipeline, `T-01a-035` list helpers, `T-01a-037` source-ready gate, `T-01a-017` accessors.

### 7.3 Priority notes

* P0 (21): the spine above, the crash reporter, the preference binding, the list helpers, the source gate and the theme.
* P1 (11): incognito, lifecycle, app lock, notification registry, single instance and deep links, `Paths` test kit, crash share and crash test hooks, tri-state widget, links constants, serialization conventions.
* P2 (7): texture-limit setting, low-RAM detection, notification privacy option, launcher badge, release profiles, FakeTracker, first-frame behaviour during migrations.
* P3 (9): drops, each with a decision and user-loss statement (section 5).

### 7.4 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| area:02 (preferences) and area:03/04 (schemas) are not ready when T-01a-011 and T-01a-014 start | The startup path is blocked; the critical path stalls | Build `Settings` with the TOML layout first; keep `Db::open` testable with an empty schema; agree the `user_version` numbers early. |
| `OnceLock` re-entrancy deadlock between services (for example source manager calling download manager during its own init) | Hang on first access | Enforce the DAG rule; use `OnceLock<Arc<dyn DownloadLookup>>` filled after both exist (`p1-topic-app-context-and-startup.md` s2); unit test T-01a-003 touches every accessor first in every order. |
| Panic inside a GTK callback is undefined behaviour unless the hook exits | Corrupted state or silent abort | Hook spawns the reporter before returning; `panic = "abort"` in release; `gtk-rs` aborts on panic in callbacks as a second safety net. |
| Crash report written by a panicking thread that holds a lock | Deadlock inside the hook | Hook reads `CrashContext` with `try_lock` only; sections are omitted when busy (`p1-topic-crash-handling.md` s2). |
| Migrations fail mid-chain and the version is not stored | Migration reruns on each launch and may keep failing | Each migration is idempotent by contract (area:11); the crash window shows the migration error text. |
| Warm-up order differs from Aniyomi when a DB is corrupt | First window opens with a fatal error | DB open failure is fatal by design; the crash window shows the SQLite error and the DB path. |
| Dev-only env hooks (`KUROBAYOMI_SMOKE`, `KUROBAYOMI_SHOT`, ...) leak into release | Surprising behaviour for the user | Document them in `--help`; keep them named with a single prefix; no behaviour change when unset. |
| `gtk4` 0.11 with feature `v4_16` requires GTK 4.16 on the user machine | Startup failure on older distributions | The user is on CachyOS (current GTK); state the minimum in the release notes (area:54). |

## 8. Definition of done for this area

Each item is observable by the user or by a test named in `p1-edge-cases-and-tests.md`.

1. `kurobayomi --version` prints `Kurobayomi 0.18.2.1 (<sha>, 132, <build time>)` (or the Cargo version if the build is not from git) and exits 0 without opening a window.
2. Starting the app creates `config`, `state`, `data`, `cache` directories under XDG paths with mode 0700 when missing. A read-only home prints an error to stderr and exits 1.
3. A corrupt `settings.toml` is moved to `settings.toml.bad-<unix_ms>`, a warning appears in the log, and the app starts with defaults.
4. Opening the main window runs the migrator once per version change. A second start with the same version runs no migration (verified by the `run` return value `Ok(false)`).
5. A fresh install creates both databases with the PRAGMAs `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL`, and `user_version` set to the schema version.
6. A deliberate panic (`KUROBAYOMI_CRASH_TEST=1`, T-01a-048) shows the "Whoops!" window with the panic message, a report file in `cache/crash/`, and buttons "Share crash logs" (copy to clipboard and reveal) and "Restart the application". No second crash occurs when the reporter itself starts.
7. A panic in a tokio task (not the GTK thread) also produces a report and exits with code 101 after the window of the reporter is shown.
8. Theme mode changes in Settings apply immediately to every open window, without restart, for LIGHT, DARK and SYSTEM.
9. Turning on incognito mode shows the banner in the main window and the notification `incognito_mode_channel:-701`; the banner's button turns incognito off; a restart with incognito still on shows the banner again.
10. Leaving the window unfocused for 700 ms or more emits a `Stopped` lifecycle event; regaining focus emits `Started`. Short focus changes (under 700 ms) emit nothing.
11. The app lock (if enabled per Q2) requires the unlock after the configured `lock_app_after` interval of inactivity and shows no content behind the lock screen.
12. Warm-up order is `network`, `manga_sources`, `anime_sources`, `manga_db`, `anime_db`, `manga_downloads`, `anime_downloads`, each logged at DEBUG with `warm-up <name> took <ms> ms`.
13. `cargo test -p kuro-core` passes the upstream migrator port (6 tests in `p1-edge-cases-and-tests.md` section 2) and the new unit tests of sections 3.1 to 3.11.
14. `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 01-foundation-app-bootstrap/p1-tasks.md` prints `ok`.
