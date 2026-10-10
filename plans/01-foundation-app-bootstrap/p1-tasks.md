# Area 01 — Implementation tasks

Format is the repository task format (lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint <file>`). Priorities follow the plan rules (P0 = needed so Kurobayomi is usable daily, P1 = parity features most users rely on, P2 = polish, P3 = Android-only/obsolete with a recorded replacement or drop). Design details live in `p1-behavior.md`, `p1-rust-design.md` and the four topic files of this folder.

### T-01a-001 — Cargo workspace with kuro-core, kuro-platform and kuro-testkit crates
- Priority: P0
- Effort: M
- Depends: none
- Sources: app/build.gradle.kts, plans/addenda/requirements-cross-platform.md
- Acceptance:
  - the repository root `Cargo.toml` is a workspace containing the existing `kurobayomi` GUI package and `crates/kuro-core`, `crates/kuro-platform`, `crates/kuro-testkit`; `cargo build` and `cargo test --workspace` succeed
  - `kuro-core` and `kuro-platform` do not depend on `gtk4`; a CI step `cargo tree -p kuro-core | grep -c gtk` prints 0
  - `rg "cfg\(target_os" crates src` finds matches only under `crates/kuro-platform`
  - the existing mockup still starts and `KUROBAYOMI_SMOKE=1 cargo run` prints the same `smoke:` summary line as before the change

### T-01a-002 — Build identity (build.rs) and AppInfo
- Priority: P0
- Effort: S
- Depends: T-01a-001
- Sources: app/build.gradle.kts, app/src/main/java/eu/kanade/tachiyomi/AppInfo.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, buildSrc/src/main/kotlin/mihon/buildlogic/Commands.kt
- Acceptance:
  - `kuro_core::app_info::APP_INFO` exposes app id `io.github.anas1412.Kurobayomi`, version name, `VERSION_CODE` (starts at 1), commit count, short commit SHA, build time in `%Y-%m-%dT%H:%M:%SZ` UTC, build type (debug, preview, release) and `updater_enabled`
  - a debug build appends `-<commit count>` to the version name and a release build does not
  - building from a source tarball without `.git` succeeds with commit count `0` and SHA `unknown`
  - `supported_image_mime_types()` returns `image/avif`, `image/gif`, `image/heif`, `image/jpeg`, `image/jxl`, `image/png`, `image/webp` in that order, minus any whose decoder feature is disabled
  - `kurobayomi --version` prints `Kurobayomi <version> (<sha>, <code>, <build time>)` and exits 0 without initialising GTK

### T-01a-003 — AppContext: the dependency-injection replacement
- Priority: P0
- Effort: L
- Depends: T-01a-001, T-01a-009, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - `AppContext::new(platform, settings, rt)` builds only the eager fields (platform, info, runtime handle, settings) and every other service is created on first accessor call through `OnceLock`
  - unit test: for every accessor, a fresh context calling only that accessor returns without panic or deadlock within 10 s
  - unit test: eight threads released by a barrier call the same accessor; the service constructor runs exactly once and all threads receive the same `Arc`
  - circular collaborators (source manager needing the download manager) are resolved through a late-filled `OnceLock<Arc<dyn Trait>>`, documented in the accessor
  - no crate other than the GUI crate and `kuro-core::bootstrap` refers to `AppContext` as a whole; services take their collaborators as constructor arguments

### T-01a-004 — main() startup order, activation and shutdown
- Priority: P0
- Effort: M
- Depends: T-01a-003, T-01a-005, T-01a-006, T-01a-009, T-01a-012, T-01a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - order verified by a startup test log: argument parsing, platform paths, logging, crash hook, settings, runtime, context, GTK application, startup signal (theme, notification categories, incognito indicator, lifecycle), first activation (migrations, window, warm-up)
  - when `--crash-report <file>` is the first argument no other step runs (no settings load, no database open, no runtime)
  - on first activation migrations run to completion before the main window is created; a second activation only presents the existing window
  - exit code of the process equals the `gtk::Application::run` return value and the tokio runtime is shut down with a 2 s timeout
  - dev environment variables `KUROBAYOMI_SMOKE`, `KUROBAYOMI_OPEN`, `KUROBAYOMI_SHEET`, `KUROBAYOMI_SIZE`, `KUROBAYOMI_SHOT`, `KUROBAYOMI_RAIL` keep their current behaviour

### T-01a-005 — Logging with tracing, verbose switch and rotating file
- Priority: P0
- Effort: M
- Depends: T-01a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt
- Acceptance:
  - ERROR and WARN records always reach `$XDG_STATE_HOME/kurobayomi/logs/kurobayomi.log` even when `verbose_logging` is false
  - with `verbose_logging = true` (default true only in debug builds) records at all levels go to stderr and the file; changing the preference at runtime updates the filter through a `reload` handle without restart
  - the log file rotates daily and only the newest 7 files are kept
  - the log line format contains an RFC 3339 timestamp with offset, the level token surrounded by spaces (for example ` ERROR `), the target and the message, so the crash dump can filter on the level token

### T-01a-006 — Panic hook, crash report file and reporter process spawn
- Priority: P0
- Effort: L
- Depends: T-01a-005, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/crash/GlobalExceptionHandler.kt, app/src/main/java/eu/kanade/tachiyomi/crash/CrashActivity.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - `crash::install` is called before any other initialisation and replaces the hook with the algorithm of `p1-topic-crash-handling.md` section 3 (log at ERROR, write report atomically, spawn `kurobayomi --crash-report <file>` detached, call previous hook, `exit(101)`)
  - a panic on the GTK main thread, a tokio worker, a `spawn_blocking` thread and a plain `std::thread` each produce exactly one report file and exit code 101
  - a second panic during reporting exits 101 without recursion and without a second file
  - `crash::contain(closure)` turns a panic into `Err` without a report and without exiting; nothing else uses `catch_unwind`
  - the report file follows format version 1 (`kurobayomi-crash-report 1`, header lines, `[facts]`, extension sections, `[backtrace]`) and old files beyond 14 days or beyond the newest 20 are deleted at normal startup
  - nothing is uploaded anywhere

### T-01a-007 — Crash reporter window
- Priority: P0
- Effort: M
- Depends: T-01a-006
- Sources: app/src/main/java/eu/kanade/presentation/crash/CrashScreen.kt, app/src/main/java/eu/kanade/tachiyomi/crash/CrashActivity.kt, presentation-core/src/main/java/tachiyomi/presentation/core/screens/InfoScreen.kt
- Acceptance:
  - `kurobayomi --crash-report <file>` opens one window with, top to bottom: bug icon (48 px, accent colour), heading `Whoops!`, subtitle `Kurobayomi ran into an unexpected error. Share the crash logs when you report the problem.`, a rounded filled box with the selectable monospace crash text, a separator, a full-width `Share crash logs` button and a full-width `Restart the application` button
  - the reporter registers application id `io.github.anas1412.Kurobayomi.CrashReporter` with non-unique flags and does not open the database, settings writer, runtime or global stylesheet
  - an unreadable or missing report file shows `No crash details available` and logs `Wasn't able to retrieve crash report` to stderr
  - Escape and the close button quit the reporter without restarting; `Restart the application` spawns the normal app detached after the crashed process has exited (polls for at most 2 s) and then quits
  - with no display available the reporter prints the report path to stderr and exits 1

### T-01a-008 — Share crash logs: dump file, clipboard and reveal
- Priority: P1
- Effort: M
- Depends: T-01a-007, area:52-util-app
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt, app/src/main/java/eu/kanade/presentation/crash/CrashScreen.kt
- Acceptance:
  - pressing `Share crash logs` writes `paths.cache()/crash/kurobayomi_crash_logs.txt` (deleting any previous file first) containing, in order, the debug block, the manga extension block (only if non-empty), the anime extension block (only if non-empty), the exception text and the last 500 ERROR lines of the log file (max 256 KiB; `(no log file)` when the file is missing)
  - the full text is copied to the clipboard and the file is revealed in the file manager; the status line shows `Crash logs copied to the clipboard` for 4 s
  - any failure shows `Failed to get logs` for 2 s and re-enables the button; the button is insensitive while the dump runs
  - golden-file test: debug block labels are `App ID`, `App version`, `OS`, `Kernel`, `Desktop`, `GTK version`, `WebView`, `Current time`, `MPV version`, `FFmpeg version`, `SQLite version`, `Locale`; the extension block lists only extensions with a newer available version code or marked obsolete, sorted by name, using `Installed: <v> / Available: <v or ?>` and `Obsolete: <bool>`

### T-01a-009 — Tokio runtime and the GTK main-loop bridge
- Priority: P0
- Effort: M
- Depends: T-01a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/mihon/core/migration/Migrator.kt
- Acceptance:
  - a multi-thread runtime with thread name `kuro-rt` and `clamp(available_parallelism, 2, 8)` workers is created in `main` and its handle is stored in `AppContext`
  - helper `ui::spawn_with_result(future, callback)` runs the future on tokio and invokes the callback on the GTK main thread; a test with a main context proves the callback runs on the main thread
  - `Handle::block_on` is called on the GTK thread only from `migrate::run_blocking`; a grep check in CI enforces this
  - shutdown waits at most 2 s for blocking tasks

### T-01a-010 — PrefBinding: GTK mirror of a preference (PreferenceMutableState)
- Priority: P0
- Effort: M
- Depends: T-01a-009, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/core/preference/PreferenceMutableState.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt
- Acceptance:
  - `PrefBinding::new` reads the current value; `get` returns the last delivered value; `set` writes through to the preference and `get` changes only after the change notification is processed
  - `bind(pref, f)` calls `f` immediately with the current value and once per delivered change; rapid consecutive writes may collapse but the last call carries the last value
  - dropping a binding stops its task; unit test with a counting closure verifies no calls after drop
  - duplicate notifications with an equal value do not break callers (documented and tested)

### T-01a-011 — Bootstrap preference groups: Base, Security, Network
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt
- Acceptance:
  - settings structs contain every key of `p1-data-and-config.md` sections 5.1 to 5.4 with the stated defaults (`downloaded_only` false, `incognito_mode` false, `onboarding_complete` false, `display_profile` empty, `hardware_bitmap_threshold` 2048, `always_decode_long_strip_with_ssiv` false, `use_biometric_lock` false, `lock_app_after` 0, `secure_screen_v2` INCOGNITO, `hide_notification_content` false, `last_app_closed` 0, `verbose_logging` true only in debug builds, `doh_provider` -1, `default_user_agent` the Firefox 136 string, `pref_theme_mode_key` SYSTEM)
  - app-state keys (`downloaded_only`, `incognito_mode`, `onboarding_complete`, `last_app_closed`, `last_version_code`) live in `state.toml` and are excluded from backup export; each stored key string is mapped to its legacy Aniyomi key (`__APP_STATE_last_app_closed`, `use_biometric_lock`, `lock_app_after`, `secure_screen_v2`, `hide_notification_content`) for backup compatibility
  - enums `ThemeMode` and `SecureScreenMode` serialize as the Kotlin constant names (`LIGHT`, `DARK`, `SYSTEM`; `ALWAYS`, `INCOGNITO`, `NEVER`); an unknown name yields the default
  - unit test: loading a file that contains only unknown keys returns all defaults and saving keeps the unknown keys

### T-01a-012 — Migrator framework (Version, Strategy, plan, run, run_blocking)
- Priority: P0
- Effort: M
- Depends: T-01a-003, T-01a-009
- Sources: app/src/main/java/mihon/core/migration/Migrator.kt, app/src/main/java/mihon/core/migration/MigrationStrategy.kt, app/src/main/java/mihon/core/migration/MigrationStrategyFactory.kt, app/src/main/java/mihon/core/migration/MigrationJobFactory.kt, app/src/main/java/mihon/core/migration/Migration.kt, app/src/main/java/mihon/core/migration/MigrationContext.kt, app/src/test/java/mihon/core/migration/MigratorTest.kt
- Acceptance:
  - `select(old, new)` returns Initial for old 0, Noop for old >= new, `Range{old+1, new}` otherwise
  - `plan` keeps always-migrations plus versions inside the inclusive range, stable-sorted with Always first
  - `run` executes sequentially, logs `Running migration: { name = <name>, version = <version> }` (dry run logs the same text prefixed with `(Dry-run) ` and executes nothing), accumulates any-true, calls `on_complete` only when the result is true, and stops with an error on the first failing migration
  - the six upstream tests are ported one-to-one (`p1-edge-cases-and-tests.md` section 2) and pass
  - `run_blocking` panics with the error chain on failure so the crash window explains it

### T-01a-013 — Migration list and last_version_code bookkeeping
- Priority: P0
- Effort: S
- Depends: T-01a-012, T-01a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/mihon/core/migration/migrations/Migrations.kt, app/src/main/java/mihon/core/migration/migrations/SetupBackupCreateMigration.kt, app/src/main/java/mihon/core/migration/migrations/SetupAnimeLibraryUpdateMigration.kt, app/src/main/java/mihon/core/migration/migrations/SetupMangaLibraryUpdateMigration.kt
- Acceptance:
  - at startup `last_version_code` (default 0) is read, `Migration from <old> to <new>` is logged and the strategy runs; on completion `Updating last version to <new>` is logged and the value is stored in `state.toml`
  - the registered list holds the three always-migrations (backup scheduler setup, anime library update scheduler setup, manga library update scheduler setup) wired to the scheduler traits of area:21-backup-format-and-create and area:26-library-update-jobs; until those exist the list is empty and an empty plan on first install still stores the version code
  - a downgrade (stored code greater than current) is a no-op, logs `Downgrade detected ({old} > {new})` and does not lower the stored value
  - the 41 legacy Aniyomi migrations (versions 7 to 130) are not ported; the drop reason is recorded in the code comment of `list.rs`

### T-01a-014 — Database open with PRAGMAs and user_version handling
- Priority: P0
- Effort: L
- Depends: T-01a-003, area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelightanime/dataanime/animehistory.sq
- Acceptance:
  - `Db::open` creates the parent directory, applies `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL` and a 5000 ms busy timeout to every pooled connection (tests read the PRAGMAs back)
  - `manga.db` and `anime.db` are separate files under `paths.data()` and each has its own `DbSpec` with schema version, create and upgrade functions
  - new file: create runs in one transaction and `user_version` equals the schema version; older file: upgrade runs in one transaction and rolls back on failure; newer file: `DbError::TooNew` with the message `Database is newer than this version of Kurobayomi (found {found}, supported {supported})`
  - `read` and `write` helpers run on the blocking pool and never block the GTK thread
  - foreign key violation test proves enforcement on connections obtained after the first

### T-01a-015 — Column adapters as rusqlite types
- Priority: P0
- Effort: S
- Depends: T-01a-014
- Sources: data/src/main/java/tachiyomi/data/DatabaseAdapter.kt, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - `UnixMillis`, `StringList`, `UpdateStrategy`, `FetchType`, `Memo` implement `ToSql` and `FromSql` with the exact encodings of `p1-data-and-config.md` section 10
  - tests: genre `a, b` splits into two items, empty string gives an empty list, a genre containing `, ` splits on read (documented sharp edge), out-of-range update strategy decodes to `ALWAYS_UPDATE`, out-of-range fetch type decodes to `Episodes`, invalid memo bytes return an error
  - used by the history, mangas, animehistory, animes and episodes row mappers of areas 05 and 06 without local copies

### T-01a-016 — Serialization conventions (Json, XML, Protobuf)
- Priority: P1
- Effort: S
- Depends: T-01a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - a documented derive macro or helper in `kuro-core::serde_conv` gives structs `#[serde(default)]` and `skip_serializing_if = "Option::is_none"` equivalent to `ignoreUnknownKeys = true` and `explicitNulls = false`
  - test: a JSON document with an extra unknown field and a missing optional field decodes; encoding omits null fields
  - the XML writer helper emits two-space indentation and a declaration naming the encoding, ignores unknown child elements when reading, and has a round-trip test on a ComicInfo sample
  - `prost-build` is wired for the backup schema of area:21-backup-format-and-create

### T-01a-017 — AppContext service accessors for every Aniyomi singleton
- Priority: P0
- Effort: M
- Depends: T-01a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - every row of the table in `p1-topic-app-context-and-startup.md` section 2 has an accessor (or an explicit "not a field" note) and a trait or stub type owned by its area so the context compiles before the other areas land
  - the list of accessors equals the Aniyomi registrations of `AppModule.kt:194-239` plus the databases, with names mapped one-to-one
  - each stub returns a typed `Unimplemented` error instead of panicking, so screens can show an error state

### T-01a-018 — Post-first-frame warm-up of expensive services
- Priority: P0
- Effort: S
- Depends: T-01a-017, T-01a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - after the main window is presented, seven idle steps run in this order: network helper, manga source manager, anime source manager, manga database, anime database, manga download manager, anime download manager
  - each step logs `warm-up <name> took <n> ms` at DEBUG
  - database steps run on the blocking pool and do not freeze the window (frame clock keeps ticking in a manual check)
  - a panic in any step reaches the crash window with the step name in the message

### T-01a-019 — ProcessLifecycle (foreground/background events with 700 ms debounce)
- Priority: P1
- Effort: S
- Depends: T-01a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - `Started` is emitted when the first application window becomes active; `Stopped` is emitted 700 ms after the last window stops being active unless a window becomes active in between
  - test with a mocked window-activity source: deactivate/reactivate within 300 ms emits no `Stopped`; deactivate for 800 ms emits exactly one `Stopped`
  - subscribers receive events through a `tokio::sync::broadcast` receiver on any thread

### T-01a-020 — Incognito indicator: window banner, notification and disable action
- Priority: P1
- Effort: M
- Depends: T-01a-010, T-01a-023, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - while `incognito_mode` is true a banner labelled `Incognito mode` with a `Disable incognito mode` button is visible in the main window and one notification `incognito_mode_channel:-701` (title `Incognito mode`, text `Disable incognito mode`) exists; both disappear when it becomes false
  - the persisted value is applied at startup
  - the application action `app.incognito-off` sets the preference to false and is also bound to the notification default action
  - banner precedence matches `MainActivity` (indexing, then downloaded-only, then incognito)
  - toggling the More-screen switch updates banner and notification without restart

### T-01a-021 — Theme mode application (LIGHT, DARK, SYSTEM)
- Priority: P0
- Effort: S
- Depends: T-01a-010, T-01a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - `theme_mode::apply` sets `gtk-application-prefer-dark-theme` to false for LIGHT and true for DARK, and restores the value captured at startup for SYSTEM; it never forces light for SYSTEM
  - the mode is applied once at startup and again on every preference change
  - no-op without a display (test passes under no display)
  - the user's GTK theme and accent are untouched (no CSS reset)

### T-01a-022 — Texture-limit preference and options list
- Priority: P2
- Effort: S
- Depends: T-01a-011, area:43-reader-core
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - `device_texture_limit()` = `max(probe, 2048)` where the probe is `GL_MAX_TEXTURE_SIZE` from a short-lived `gdk::GLContext` (fallback 16384 when no GL context is possible) and `KUROBAYOMI_TEXTURE_LIMIT` overrides it
  - `custom_texture_limit_options(device)` reproduces the Kotlin algorithm; tests: 16384 gives the 15 values 16384 down to 2048 in steps of 1024, 8192 gives 7, 4096 gives `4096, 3072, 2048`, 3000 gives `3000, 2048`, 2048 gives `2048`
  - at first launch `pref_hardware_bitmap_threshold` is stored as the device limit when unset; later launches leave it alone
  - the stored value is exposed to the reader as an atomic read; changes propagate without restart

### T-01a-023 — Notification category registry and Notifier mapping
- Priority: P1
- Effort: M
- Depends: T-01a-001, area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - `CATEGORIES` holds the 13 channels of `p1-data-and-config.md` section 9 with id, group, importance, name key and flags; unit test compares ids to the list
  - helper `key(channel, id)` returns `"{channel}:{id}"`; the two colliding Aniyomi ids (`2` on `common_channel` and on `app_apk_update_channel`) map to distinct keys
  - the Linux `Notifier` maps Low to urgency low, Default and High to normal, `silent` to suppress-sound, and sets the spec category hint per channel as listed in `p1-rust-design.md` 3.7
  - registration errors are logged `Failed to modify notification channels` and never fatal

### T-01a-024 — Drop home-screen widget managers (decision and sidebar badge source)
- Priority: P3
- Effort: S
- Depends: area:50-android-widgets
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeWidgetManager.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - no widget code is ported; the code base contains a note in `docs` stating the drop reason (no home-screen widget surface on desktop) and what the user loses (home-screen cover grid, locked widget state)
  - the unread-updates count used by the sidebar badge is produced by one query function "unread updates created in the last 3 months" shared with area:42-updates-history-stats-storage-upcoming
  - the sidebar badge literals `"8"` and `"1"` in `src/main.rs` are replaced by that count once area:42 delivers it (tracked by this task's dependency)

### T-01a-025 — Launcher badge for unread updates (optional)
- Priority: P2
- Effort: S
- Depends: T-01a-024, area:53-platform-manifest-intents-workers
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt
- Acceptance:
  - the unread-updates count (3-month window) is published through the `com.canonical.Unity.LauncherEntry` D-Bus signal when the desktop file name is known
  - the badge is cleared when the count is 0 and hidden while the app lock is enabled (parity with the locked widget state)
  - failure to reach the bus is logged at DEBUG and ignored

### T-01a-026 — Drop WebView package-name spoofing (getPackageName override)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt
- Acceptance:
  - decision recorded: Android WebView sends `X-Requested-With: <package name>` and Aniyomi swaps it for the Chrome package; WebKitGTK sends no such header so no code is written
  - area:12-network-stack and area:36-ui-shell-navigation-security-webview are told (by their task text) that any header policy for challenge solving is theirs
  - the user loses nothing

### T-01a-027 — Drop Conscrypt TLS provider insertion
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/build.gradle.kts
- Acceptance:
  - decision recorded: Aniyomi inserts Conscrypt for TLS 1.3 on Android below 10; reqwest with rustls provides TLS 1.2 and 1.3 on every Linux system
  - `cargo tree` shows no OpenSSL dependency in the default feature set
  - the user loses nothing

### T-01a-028 — Drop WebView data-directory suffix
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - decision recorded: the suffix avoids two Android processes sharing one WebView data directory; Kurobayomi's crash reporter never creates a web view and the main app is single-process
  - the WebKit data and cache directories are `paths.data()/webkit` and `paths.cache()/webkit` (set by the `WebViewHost` implementation)
  - the user loses nothing

### T-01a-029 — Image pipeline concurrency, cross-fade and low-RAM settings
- Priority: P0
- Effort: M
- Depends: T-01a-003, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt
- Acceptance:
  - the image pipeline created by `AppContext` has a fetch semaphore of 8 permits and a decode semaphore of 3 permits (test: the ninth concurrent fetch waits, the fourth concurrent decode waits)
  - cross-fade duration is 300 ms when `gtk-enable-animations` is true and 0 otherwise
  - component order is preserved in spirit: generic network fetch, image decode, buffered-source fetch, manga cover, manga cover (library), anime image, anime cover, then the four keyers (anime, manga, anime cover, manga cover)
  - the pipeline is created lazily on first image request and the HTTP client is resolved lazily inside it

### T-01a-030 — Low-RAM detection
- Priority: P2
- Effort: S
- Depends: T-01a-029
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt
- Acceptance:
  - `device::is_low_ram()` returns true when `MemTotal` in `/proc/meminfo` is below 3 GiB (3 221 225 472 bytes) and false otherwise; a missing or unparsable file returns false
  - the result is cached for the process lifetime
  - the image pipeline reads it to shrink its in-memory cache (value owned by area:34-images-covers-cache)

### T-01a-031 — Links and deep-link constants
- Priority: P1
- Effort: S
- Depends: T-01a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt
- Acceptance:
  - `links::URL_HELP = "https://aniyomi.org/docs/guides/troubleshooting/"` and `links::URL_HELP_UPCOMING = "https://aniyomi.org/docs/faq/updates/upcoming"` exist and are opened through the `Opener` trait
  - `DeepLink` has the eleven variants of `p1-data-and-config.md` section 7 with `parse`, `name` and `action_name`; `Manga(id)` and `Anime(id)` require an id and `parse("manga")` is `None`
  - tests cover every variant round trip and the invalid inputs `manga=abc` and an unknown name

### T-01a-032 — App lock state machine
- Priority: P1
- Effort: M
- Depends: T-01a-019, T-01a-011, area:36-ui-shell-navigation-security-webview
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - a pure function implements `onApplicationStopped` and `onApplicationStart` with the rules of `p1-behavior.md` section 3.1; the 13-row table test of `p1-edge-cases-and-tests.md` section 3.9 passes
  - `require_unlock` starts true at process start; `unlock()` sets it false; `lock_app_after`: -1 never, 0 always, N minutes using 64-bit arithmetic; `last_app_closed` is written only when `lock_app_after > 0` and deleted on every start
  - a clock moved backwards (`now < last_closed`) locks (documented deviation)
  - when no unlock method is configured on resume, `use_biometric_lock` is set to false silently

### T-01a-033 — Drop secure-screen mode (FLAG_SECURE)
- Priority: P3
- Effort: S
- Depends: T-01a-011
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt
- Acceptance:
  - the preference `secure_screen_v2` is kept for backup round trips but no UI row is shown and no code acts on it
  - decision recorded: Wayland and X11 offer no per-window "hide from screenshots and window switcher" flag; the user loses screenshot blocking and thumbnail hiding in the task switcher
  - the settings screen of area:47-settings-screens-general omits the row (cross-reference in that area's task list)

### T-01a-034 — Hide notification content preference
- Priority: P2
- Effort: S
- Depends: T-01a-023, T-01a-011
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt
- Acceptance:
  - `Notifier::send` takes a `sensitive` flag; when `hide_notification_content` is true and the flag is set the title keeps only the app name and the body is replaced by the generic text of that notification category
  - the library-update and download notifiers of areas 23, 24 and 26 pass `sensitive = true`; the incognito notification and crash messages pass false
  - unit test with a fake notifier proves substitution

### T-01a-035 — List helpers: insert_separators and insert_separators_reversed
- Priority: P0
- Effort: S
- Depends: T-01a-001
- Sources: app/src/main/java/eu/kanade/core/util/CollectionUtils.kt
- Acceptance:
  - both functions return the layout in which every item is preceded by the optional separator generated for the pair (previous item or none, this item) and the last item is followed by the optional separator generated for the pair (last item, none) and an empty list for empty input
  - forward generator call order is `(None, first)` through `(last, None)`; reversed order is the exact opposite; tests assert both orders on `["a1","a2","b1"]`
  - `addOrRemove`, `fastFilterNot`, `fastPartition`, `fastCountNot` are not ported; call sites use std (`insert`/`remove`, `filter`, `partition`, `filter().count()`), recorded in a comment in `collections.rs`

### T-01a-036 — Tri-state check widget
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/core/preference/CheckboxState.kt, core/common/src/main/java/tachiyomi/core/common/preference/CheckboxState.kt
- Acceptance:
  - `tri_check(initial, on_change)` shows Off for None, a tick for Include and a dash (inconsistent) for Exclude and cycles None, Include, Exclude, None on click, Space and Enter
  - the inline implementation in `src/ui.rs` (`Item::Tristate`) is replaced by this widget with identical looks
  - programmatic `set_state` does not fire `on_change`

### T-01a-037 — Source-ready gate and loading placeholder
- Priority: P0
- Effort: S
- Depends: T-01a-009, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/core/util/MangaSourceUtil.kt, app/src/main/java/eu/kanade/core/util/AnimeSourceUtil.kt, domain/src/main/java/tachiyomi/domain/source/manga/service/MangaSourceManager.kt
- Acceptance:
  - `SourceGate::is_initialized()` returns a `watch::Receiver<bool>` that is false from process start until the source manager finished its first load
  - `when_sources_ready` shows a centered spinner page until the gate is true and then switches to the content page with a 150 ms crossfade; if the gate is already true no spinner is created
  - applied by the 12 call sites of the Kotlin gate (browse source, global search, source preferences, migration search and season selection, manga and anime)

### T-01a-038 — Drop the RxJava bridge
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/core/util/RxJavaExtensions.kt, app/build.gradle.kts
- Acceptance:
  - decision recorded: `Observable.asFlow()` only bridged RxJava 1 in legacy extension APIs; Rust sources are async functions or streams
  - area:15-source-api-manga and area:16-source-api-anime tasks reference this decision for their `Observable`-based fetch methods
  - the user loses nothing

### T-01a-039 — FakeTracker test double
- Priority: P2
- Effort: S
- Depends: T-01a-045, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/test/DummyTracker.kt
- Acceptance:
  - `kuro-testkit::FakeTracker` implements the tracker trait of area:27 with the defaults of `p1-edge-cases-and-tests.md` section 3.12 (logo colour rgb(18,25,35), statuses 1 to 6, completion status 2, score list "0" to "10", 10-point score 5.4, username `username`, password `passw0rd`, not logged in, empty search results)
  - `client()` returns an error value rather than panicking
  - used by at least one track-dialog model test in area:27

### T-01a-040 — Cargo release, preview and dev profiles; feature flags
- Priority: P2
- Effort: S
- Depends: T-01a-001
- Sources: app/build.gradle.kts, buildSrc/src/main/kotlin/mihon/buildlogic/BuildConfig.kt
- Acceptance:
  - root `Cargo.toml` defines profile `release` (thin LTO, one codegen unit, line-tables-only debug info, no strip), profile `preview` inheriting release, and `dev` with line-tables-only debug info
  - feature `updater` (off by default) controls `updater_enabled`; the build type reported by `app_info` matches the profile
  - a release binary produces file and line numbers in a panic backtrace (checked by the crash end-to-end test run in release mode)
  - Android-only knobs (ABI splits, benchmark build type, lint settings, Kotlin opt-ins, dependency info) are listed as dropped in the build documentation

### T-01a-041 — Drop R8 rules; hand the extension host-API checklist to area:20
- Priority: P3
- Effort: S
- Depends: area:20-extension-strategy-redesign
- Sources: app/proguard-rules.pro, app/proguard-android-optimize.txt
- Acceptance:
  - decision recorded: shrinker rules are not applicable to Rust; the user loses nothing
  - the host-API checklist of `p1-topic-build-and-dependency-map.md` section 5 (HTTP client with rate limit helpers, Cloudflare client, default user agent, request builders, HTML and JSON parsing, per-source preference screen, `AppInfo`, QuickJS evaluation, mpv/ffmpeg helpers, torrent helpers) is referenced by the area:20 task that defines the plugin ABI
  - the task is closed when area:20 has a task covering each checklist row

### T-01a-042 — Single instance, command line and deep-link dispatch
- Priority: P1
- Effort: M
- Depends: T-01a-031, T-01a-004, area:36-ui-shell-navigation-security-webview
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - starting a second `kurobayomi --open updates` forwards the command to the running instance (which opens the Updates tab) and exits 0
  - `--open manga=<id>` and `--open anime=<id>` pop to the root, switch to the library tab and open that entry; a missing id is rejected with exit code 2 and a stderr message
  - the eleven application actions `app.open-*` exist and are referenced by the `.desktop` file `Actions=` entries (area:53-platform-manifest-intents-workers) and by notification default actions
  - dev environment variables still force a non-unique instance

### T-01a-043 — First-frame behaviour while migrations run
- Priority: P2
- Effort: S
- Depends: T-01a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - if `run_blocking` takes longer than 300 ms a small undecorated window with the app name and a spinner is shown and removed when the main window is presented
  - when migrations finish faster than 300 ms no splash window is created
  - the splash window never blocks keyboard focus handoff to the main window

### T-01a-044 — Drop the extension-installer preference
- Priority: P3
- Effort: S
- Depends: area:20-extension-strategy-redesign
- Sources: app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - decision recorded: LEGACY, PACKAGEINSTALLER, SHIZUKU and PRIVATE are Android package-installer strategies; Kurobayomi's extension install path is defined by area:20
  - the legacy key `extension_installer` is accepted on backup import and ignored
  - the user loses nothing on Linux

### T-01a-045 — kuro-testkit: TempPaths, InMemorySettings and fixtures
- Priority: P1
- Effort: M
- Depends: T-01a-001, area:02-preferences-system
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/InMemoryPreferenceStore.kt, app/src/test/java/mihon/core/migration/MigratorTest.kt
- Acceptance:
  - `TempPaths` implements `Paths` over a `tempfile::TempDir`; `InMemorySettings` implements the settings contract with change notification (the Kotlin in-memory store's `changes()` never emitted and `getStringSet` was unimplemented; the Rust one must do both)
  - fixtures listed in `p1-edge-cases-and-tests.md` section 4 exist and are loadable through helper functions
  - every test in this area uses these helpers and none writes outside a temp directory

### T-01a-046 — Drop the Picture-in-Picture capability flag
- Priority: P3
- Effort: S
- Depends: area:46-player-controls-ui
- Sources: app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - decision recorded: `deviceHasPip()` reports Android `FEATURE_PICTURE_IN_PICTURE`; GTK has no PiP; the replacement is the always-on-top mini player window designed in area:46-player-controls-ui
  - no PiP setting is shown on Linux
  - the user loses system-level PiP; the mini player is the substitute

### T-01a-047 — Settings load, corrupt-file recovery and first-run directories
- Priority: P0
- Effort: S
- Depends: T-01a-011, area:02-preferences-system
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt, app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt
- Acceptance:
  - config, state, data and cache directories are created with mode 0700 when missing; failure is fatal with a clear stderr message
  - a syntactically invalid `settings.toml` is renamed to `settings.toml.bad-<unix_ms>`, defaults are used and a WARN is logged; a per-key type mismatch deletes the key, returns the default and logs `Invalid value for {key}; deleting`
  - unknown keys survive save; writes are atomic (temp file, fsync, rename)

### T-01a-048 — Crash test hooks and reporter dry-run switch
- Priority: P1
- Effort: S
- Depends: T-01a-006, T-01a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/crash/GlobalExceptionHandler.kt
- Acceptance:
  - `KUROBAYOMI_CRASH_TEST=1` panics with message `crash test` 500 ms after the main window is presented; `KUROBAYOMI_CRASH_TEST=worker` panics inside a tokio task
  - `KUROBAYOMI_REPORTER_DRY=1` makes the reporter print the report path and exit 0 without opening a window (used by the integration test)
  - both variables are ignored in release builds unless `KUROBAYOMI_ALLOW_TEST_HOOKS=1` is also set, and are documented in `--help`
