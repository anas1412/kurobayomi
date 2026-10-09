# Topic: crash handling (GlobalExceptionHandler, CrashActivity, CrashScreen, CrashLogUtil)

Sources: `app/src/main/java/eu/kanade/tachiyomi/crash/GlobalExceptionHandler.kt`, `app/src/main/java/eu/kanade/tachiyomi/crash/CrashActivity.kt`, `app/src/main/java/eu/kanade/presentation/crash/CrashScreen.kt`, `app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt`, `app/src/main/AndroidManifest.xml:130-133`, `app/src/main/java/eu/kanade/tachiyomi/App.kt:86`. Observed behaviour is in `p1-behavior.md` section 12; this file is the Rust design.

## 1. Parity table

| Aniyomi behaviour | Kurobayomi |
|---|---|
| `Thread.setDefaultUncaughtExceptionHandler` installed in `App.onCreate` step 3 | `std::panic::set_hook` installed in `main()` right after logging init (any thread, any panic) |
| Handler logs the throwable at ERROR | Hook logs at ERROR with target `crash` (always on, independent of verbose preference) |
| Handler starts `CrashActivity` in a separate OS process with the throwable text in an Intent extra | Hook writes a crash report file, spawns `kurobayomi --crash-report <file>` as a detached child process |
| Handler then calls the previous default handler, which kills the process | Hook calls the previous hook (prints the standard panic message and backtrace to stderr), then `std::process::exit(101)` |
| Crash screen: bug icon, "Whoops!", description, exception text, two buttons | Same elements in a GTK window (`p1-ui-parity.md`) |
| "Share crash logs" builds `aniyomi_crash_logs.txt` in the external cache and opens the Android share sheet | Builds `kurobayomi_crash_logs.txt` in `paths.cache()/crash/`, copies text to the clipboard, reveals the file in the file manager |
| "Restart the application" finishes the task and starts `MainActivity` | Spawns the normal app and quits the reporter |
| Failure to read the throwable shows the text `null` | Shows `No crash details available` (deliberate improvement; behaviour of the Kotlin is documented, the Rust text is a new string key `crash_report_unavailable`) |
| Native crashes (SIGSEGV in libmpv, GTK) are not handled (the Java handler never sees them) | Not handled either (non-goal; section 9) |

## 2. Crash context (facts captured before a crash)

The panic hook must not do heavy work or take locks that a panicking thread may already hold. Everything that is expensive to compute is captured earlier into a process-wide `CrashContext`:

```rust
pub struct CrashContext {
    facts: Mutex<BTreeMap<&'static str, String>>,   // "app_id", "app_version", "os", "kernel", "desktop", "session", "gtk", "webview", "mpv", "ffmpeg", "sqlite", "locale"
    sections: Mutex<BTreeMap<&'static str, String>>, // "manga_extensions", "anime_extensions" (already formatted text, may be empty)
    paths: Paths,                                   // state dir, cache dir, log file path
}
pub fn set_fact(key: &'static str, value: String);
pub fn set_section(key: &'static str, text: String);
```
* `crash::install` fills `app_id`, `app_version` (`"{VERSION_NAME} ({COMMIT_SHA}, {VERSION_CODE}, {BUILD_TIME})"`), `os` (`PRETTY_NAME` from `/etc/os-release`, fallback `ID`, fallback `unknown`), `kernel` (`uname -sr` through `rustix::system::uname`), `desktop` (`XDG_CURRENT_DESKTOP`, fallback `unknown`), `session` (`XDG_SESSION_TYPE`, fallback `unknown`), `locale` (`LC_ALL`, else `LC_MESSAGES`, else `LANG`, else `unknown`), `sqlite` (`rusqlite::version()`).
* GTK init adds `gtk` = `"{major}.{minor}.{micro}"` from `gtk::{major,minor,micro}_version()`.
* The WebView host, when first created, sets `webview` = `"WebKitGTK <version>"`; until then the line prints `not loaded`.
* The player core, when libmpv initialises, sets `mpv` from the `mpv-version` property and `ffmpeg` from `ffmpeg-version`; until then `not loaded`. (Aniyomi prints three hard-coded constants `MPV version: 6764488`, `Libplacebo version: v7.349.0`, `FFmpeg version: n7.1`, with a source comment saying they should come from `Utils.VERSIONS`; the Rust app reads real values. libmpv exposes no libplacebo version property, so that line is not printed.)
* The extension managers call `set_section` whenever their installed/available lists change, using the formatting of section 6.3. The hook only reads with `try_lock`; if a lock is busy the section is omitted from the report.

## 3. Panic hook algorithm

```rust
pub fn install(paths: &Paths) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if SUPPRESS.with(|s| s.get() > 0) { return; }                  // expected panic inside catch_unwind
        if CRASHING.swap(true, Ordering::SeqCst) { std::process::exit(101); }  // second panic while reporting
        let report = build_report(info);                               // never panics: uses unwrap_or_else defaults
        tracing::error!(target: "crash", "{}", report.summary());
        let file = write_report_file(&report);                         // atomic write; Err is logged
        match file.and_then(|f| spawn_reporter(&f)) {
            Ok(()) => {}
            Err(e) => eprintln!("kurobayomi: could not start the crash reporter: {e}"),
        }
        prev(info);                                                    // prints standard message to stderr
        std::process::exit(101);
    }));
}
```
Steps in `build_report(info)`:
1. `time` = `OffsetDateTime::now_local()` falling back to UTC (`time` crate), formatted RFC 3339 with offset (Aniyomi prints `OffsetDateTime.now(ZoneId.systemDefault())`).
2. `thread` = `std::thread::current().name()` or `"unnamed"`.
3. `location` = `info.location()` as `file:line:column`, else `unknown`.
4. `message` = payload downcast to `&'static str`, else `String`, else the literal `Box<dyn Any>`.
5. `backtrace` = `std::backtrace::Backtrace::force_capture().to_string()` (works even when `RUST_BACKTRACE` is unset).
6. `facts` and `sections` from `CrashContext` using `try_lock`.
Panics of any thread (GTK main, tokio worker, blocking pool, `std::thread::spawn`) reach the hook. Tokio would otherwise convert a task panic into a `JoinError` and keep the process alive; the hook runs first, so the app crashes, matching Android where an uncaught coroutine exception crashes the process.

Expected panics: code that deliberately contains panics from untrusted third-party parsers (image decoders on corrupt pages, archive readers, JSON from sources) wraps the call in `crash::contain(|| ...)`, which increments a thread-local `SUPPRESS` counter around `catch_unwind` and returns `Result<T, PanicMessage>`; the hook returns immediately when the counter is positive, and the panic is logged at WARN by `contain`. No other code uses `catch_unwind`.

`spawn_reporter(file)`:
```rust
let exe = std::env::current_exe().or_else(|_| std::fs::read_link("/proc/self/exe"))?;
let mut cmd = Command::new(exe);
cmd.arg("--crash-report").arg(file).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
cmd.process_group(0);        // detach from the parent's process group so it survives the exit below
cmd.spawn()?;                // not waited on
```
(`process_group` is in `std::os::unix::process::CommandExt` and therefore lives in `kuro-platform::spawn_detached` to respect the no-OS-code-elsewhere rule.) The child inherits `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`, `GTK_THEME` and every other environment variable, so it renders with the same theme. A deleted binary (`/proc/self/exe` reads `... (deleted)` after an in-place upgrade) is handled by the fallback; if both fail the error is printed to stderr.

Why exit instead of letting the panic unwind: with `panic = "unwind"` a panic in a worker thread would otherwise leave the app running in a half-broken state; Android's default handler also kills the process. Exit code 101 equals Rust's default for a main-thread panic.

## 4. Crash report file

Directory `paths.state()/crashes/`, file name `crash-<unix_ms>.txt`, UTF-8, written to `crash-<unix_ms>.txt.tmp` then `fsync` then `rename` (the reporter never sees a partial file). Format version 1:

```
kurobayomi-crash-report 1
time: 2026-10-08T14:03:22.123+03:00
thread: kuro-rt
location: crates/kuro-core/src/foo.rs:42:9
message: index out of bounds: the len is 3 but the index is 7
[facts]
app_id: io.github.blackbox.Kurobayomi
app_version: 0.1.0 (a1b2c3d, 1, 2026-10-08T11:00:00Z)
os: CachyOS Linux
kernel: Linux 7.2.9-1-cachyos
desktop: KDE
session: wayland
gtk: 4.16.12
webview: not loaded
mpv: not loaded
ffmpeg: not loaded
sqlite: 3.46.0
locale: en_US.UTF-8
[manga_extensions]
Problematic extensions:
- Some Source
  Installed: 1.4.20 / Available: 1.4.25
  Orphaned: false
[anime_extensions]
[backtrace]
   0: std::panicking::begin_panic_handler
   1: core::panicking::panic_bounds_check
   2: kuro_core::foo::bar
             at ./crates/kuro-core/src/foo.rs:42:9
```
Rules: the first line is the magic and version; lines before `[facts]` are `key: value` (newlines in `message` are escaped as the two characters `\n`); each section header is a line consisting of `[name]`; a section body runs until the next header; unknown section names are preserved when displayed raw. A reader that does not recognise the magic shows the whole file as plain text. Retention: at normal startup delete crash files older than 14 days and keep at most the newest 20.

## 5. Reporter mode (replaces `CrashActivity`)

`crash_window::run(path)`:
1. `gtk::init()`; create `gtk::Application` with id `io.github.blackbox.Kurobayomi.CrashReporter` and flags `NON_UNIQUE` (so it never collides with the main instance's D-Bus name; equivalent of the separate `:error_handler` process). No settings, DB, runtime, logging file, migrator, or crash hook other than `eprintln` is initialised. Theme mode is applied by reading `settings.toml` directly with a minimal parser if present (best effort; failure ignored) so the window matches the user's light/dark choice; otherwise GTK's own settings apply.
2. Read the file. Success: show the parsed text. Failure (missing, unreadable, empty): log `Wasn't able to retrieve crash report` (same wording as Kotlin's `Wasn't able to retrieve throwable from intent`) to stderr and show the fallback text.
3. Present the window (`p1-ui-parity.md` section 2).
4. Buttons: "Share crash logs" (section 6), "Restart the application" (section 7).
5. Closing the window (title-bar close, Escape, Alt+F4) quits the reporter without restarting (Android: back from CrashActivity closes the task).

## 6. "Share crash logs" (replaces `CrashLogUtil.dumpLogs`)

Runs on a `std::thread` (not tokio; the reporter has no runtime) and reports back through `glib::MainContext::invoke`. The button is disabled while running; success or failure re-enables it.

1. `dir = paths.cache()/crash`; `create_dir_all`; `file = dir/kurobayomi_crash_logs.txt`; delete if exists; create (Android `createFileInCacheDir`).
2. Append, each followed by two newlines: (a) debug info block, (b) manga extensions block if non-empty, (c) anime extensions block if non-empty, (d) the exception text exactly as displayed in the window.
3. Append the error log tail: last 500 lines of `paths.state()/logs/kurobayomi.log` whose level token is ` ERROR ` (replaces `logcat *:E -d -v year -v zone`; the timestamps in the tracing format already carry date and offset). Cap the tail at 256 KiB (drop the oldest lines first). If the log file is missing append the single line `(no log file)`.
4. Copy the whole file text to the clipboard (`gdk::Display::default().clipboard().set_text`, must run on the main thread).
5. `Opener::reveal(file)`: call `org.freedesktop.FileManager1.ShowItems([file_uri], "")` over the session bus; on failure fall back to `gio::AppInfo::launch_default_for_uri` on the parent directory.
6. Show the message `crash_logs_saved` ("Crash logs copied to the clipboard") in an inline label for 4 s.
7. Any error in steps 1-5 (I/O error, clipboard unavailable): show `crash_logs_failed` ("Failed to get logs") for 2 s (Android `toast` default is `Toast.LENGTH_SHORT` = about 2 s) and log the error.

Debug info block (this is the Linux version of `getDebugInfo()`; label order mirrors Aniyomi):
```
App ID: {app_id}
App version: {app_version}
OS: {os}
Kernel: {kernel}
Desktop: {desktop} ({session})
GTK version: {gtk}
WebView: {webview}
Current time: {time of the dump, RFC 3339 with offset}
MPV version: {mpv}
FFmpeg version: {ffmpeg}
SQLite version: {sqlite}
Locale: {locale}
```
Extension block text (both kinds; the Kotlin text differs only in `Orphaned:` vs `Obsolete:`; Kurobayomi uses `Obsolete:` for both):
```
Problematic extensions:
- {name}
  Installed: {installed.version_name} / Available: {available.version_name or "?"}
  Obsolete: {is_obsolete}
```
Entries: installed extensions sorted by name (Rust `str` byte order; Kotlin used UTF-16 order, identical for BMP text), included only when `available.version_code > installed.version_code` (a missing available entry counts as 0) or `is_obsolete`; entries joined with `\n`; empty list gives no block. Because Kurobayomi's extension strategy is decided in area:20-extension-strategy-redesign, the formatter takes a neutral `ExtensionStatus { name, installed_version_name, installed_version_code, available_version_name: Option<String>, available_version_code: Option<u64>, is_obsolete }` slice and area:20 provides the list.

## 7. "Restart the application"
`Command::new(exe)` with no arguments, detached via `spawn_detached`, then `application.quit()`. If spawning fails: show `crash_restart_failed` ("Could not restart the application") and keep the window open.

## 8. Error paths and edge cases

| Situation | Behaviour |
|---|---|
| Panic while writing the report file (disk full, read-only state dir) | The error is logged; `prev(info)` still prints the message; no reporter is started because there is no file; process exits 101 |
| Panic inside the hook (bug in `build_report`) | `CRASHING` is already true, the second panic invokes the hook again, sees `CRASHING`, exits 101 |
| Two threads panic simultaneously | The first sets `CRASHING`; the second exits immediately with 101 (only one report) |
| Panic before `install` (argument parsing) | Default Rust handler (stderr); the order in `main()` keeps this window to the first three lines |
| Reporter itself panics | The reporter installs only a stderr hook; no recursion |
| Report file larger than 1 MiB | The reporter truncates the displayed text to the first 1 MiB with a trailing line `(truncated)`; the dump keeps the full file |
| Display unavailable (SSH session, no `DISPLAY`/`WAYLAND_DISPLAY`) | `gtk::init()` fails in the reporter; it prints the report path to stderr and exits 1; the main process already printed the panic to stderr |
| Crash in the user's first 2 seconds (migrator) | Same path; the migrator's completion listener did not run, so `last_version_code` is unchanged and the migration retries on next launch |

## 9. Non-goals and privacy
* No upload, no telemetry (Aniyomi's ACRA block is commented out in `app/build.gradle.kts:30-40`). Crash files stay on disk until the cleanup rule removes them.
* Reports contain the home directory path (in backtraces and file paths) and may contain titles that appear in panic messages; the screen text tells the user nothing about redaction because the user decides what to share. The plan does not add redaction.
* Native crashes (SIGSEGV, stack overflow, SIGABRT from GLib `g_error`) bypass the Rust hook. Not covered in Aniyomi either (the Java handler does not see native crashes). A future "previous run ended unexpectedly" marker file is intentionally not planned.

## 10. Test hook
`KUROBAYOMI_CRASH_TEST=1` makes `main` panic with message `crash test` 500 ms after the main window is presented (checked in `build_ui`), and `KUROBAYOMI_CRASH_TEST=worker` panics inside a tokio task instead. Used by the manual test in `p1-edge-cases-and-tests.md` and by the smoke script.
