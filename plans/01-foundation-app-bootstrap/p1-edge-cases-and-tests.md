# Area 01 — Edge cases, failure modes, races, tests

## 1. Edge cases and failure modes

### 1.1 Startup and context
| # | Situation | Aniyomi (OBSERVED) | Kurobayomi decision |
|---|---|---|---|
| 1 | `settings.toml` is syntactically invalid | not applicable (SharedPreferences); an invalid typed value is deleted and replaced by the default (`AndroidPreference.get`, `"Invalid value for $key; deleting"`) | Rename the file to `settings.toml.bad-<unix_ms>`, start with defaults, log WARN. Per-key type mismatch: delete the key, return the default, log `Invalid value for {key}; deleting` (same text) |
| 2 | Unknown keys in `settings.toml` / `state.toml` (newer version wrote them) | n/a | Preserved on save (round trip through a generic table) |
| 3 | Database file has `user_version` greater than the schema this binary knows | n/a (Android would crash in SQLDelight `onDowngrade`) | `DbError::TooNew`, panic with the message so the crash window explains it; file untouched |
| 4 | Database directory not writable | Android always writable | Fatal at startup: `panic!("cannot open {path}: {err}")` shown by the crash window |
| 5 | Two instances start at once | Android single task | `gio::Application` unique id makes the second process forward its command line and exit; dev env vars set `NON_UNIQUE` (existing behaviour) |
| 6 | `OnceLock` re-entrancy (accessor A needs accessor B which needs A) | Injekt cycles would stack-overflow; Aniyomi avoids them with `injectLazy` | Keep the dependency graph a DAG; unit test builds every accessor first on a fresh context; cycles are broken with `OnceLock<Arc<dyn Trait>>` filled afterwards |
| 7 | Warm-up step panics (e.g. network helper cannot read CA roots) | Uncaught exception in `execute` block crashes the app via `GlobalExceptionHandler` | Same: panic reaches the hook; message names the step |
| 8 | Migration throws | Crash screen; `last_version_code` unchanged; retried next launch | Same (`run_blocking` panics with the error chain) |
| 9 | Every migration returns false | Listener not called, version not stored | Same; plus Kurobayomi stores the version explicitly when the plan is empty (T-01a-013) |
| 10 | `last_version_code` greater than the binary's code (user downgraded) | `old >= new` → Noop | Noop; log INFO `Downgrade detected ({old} > {new})`; do not lower the stored value |
| 11 | `git` is not installed or the source is a tarball without `.git` at build time | BuildConfig fields come from `git` commands run by Gradle (they would fail the build) | `COMMIT_COUNT = "0"`, `COMMIT_SHA = "unknown"`, `BUILD_TIME` = current UTC time; the build must not fail |

### 1.2 Crash handling
| # | Situation | Behaviour |
|---|---|---|
| 1 | Panic while holding the DB writer mutex | `Mutex` poisoned; the process exits in the hook anyway; WAL file recovers on next open |
| 2 | Panic in a GTK signal handler | Hook runs first and reports; then `exit(101)` prevents unwinding through C frames |
| 3 | Panic inside `crash::contain` | Hook returns immediately (suppression counter), the panic becomes `Err`, logged WARN `contained panic: {msg}` |
| 4 | `crash::contain` nested inside another `contain` | Counter is a depth count; correct |
| 5 | Panic during `build_report` (for example `Backtrace` capture failing) | `CRASHING` is set; the second panic exits with 101; no report file; stderr still has the standard message |
| 6 | State directory missing at crash time | The hook `create_dir_all`s `crashes/`; failure only prevents the reporter |
| 7 | Reporter cannot open a display | Prints `crash report saved to <path>` to stderr and exits 1 |
| 8 | Crash file has unknown magic or version | Shown verbatim as text |
| 9 | Report text over 1 MiB | Displayed truncated with `(truncated)`; dump keeps the whole file |
| 10 | "Share crash logs" pressed twice quickly | Second press ignored while insensitive (Aniyomi allowed concurrent dumps; the file is deleted and recreated each time so the second dump could race the first) |
| 11 | Clipboard not available (no display server clipboard manager) | Status line `Failed to get logs`; file is still written and revealed |
| 12 | `Opener::reveal` fails | Fall back to opening the parent directory; if that fails the status line still says `Crash logs copied to the clipboard` when the clipboard step worked |
| 13 | Extension info lock held by the panicking thread | Section omitted (try_lock) |
| 14 | `RUST_BACKTRACE=0` in the environment | Report still has a backtrace (`force_capture`) |
| 15 | Crash on the reporter's first frame (GTK bug) | Reporter has a plain stderr hook; no recursion, no loop of reporters (the reporter never spawns another reporter) |
| 16 | Stack overflow, SIGSEGV from libmpv/GTK, SIGABRT from GLib | Not intercepted; process dies without a screen (parity with Aniyomi native crashes) |

### 1.3 Incognito, theme, lifecycle, lock
| # | Situation | Behaviour |
|---|---|---|
| 1 | `incognito_mode` persisted true from the last session | Banner and notification shown again at startup (Aniyomi: flow emits stored value at collection start) |
| 2 | Notification service not running | Banner still works; notification failure logged at DEBUG |
| 3 | User toggles incognito 50 times quickly | `watch` conflates; final state wins; notification send/withdraw are idempotent |
| 4 | `ThemeMode::System` selected after `Dark` | The initial GTK value captured at startup is restored (`prefer-dark-theme` back to its original value), never forced to light |
| 5 | `gtk::Settings::default()` is `None` (tests without display) | `apply` is a no-op |
| 6 | Dialog opens and the main window loses `is-active` for under 700 ms | No `Stopped` event |
| 7 | App lock `lock_app_after = 0`, app returns from background | `requireUnlock = true` → unlock dialog every return |
| 8 | `lock_app_after = N > 0` and the clock moves backwards before return | Aniyomi: `last + N*60000 <= now` stays false until the clock catches up, so the app remains unlocked (weakness). Kurobayomi: if `now < last_closed` treat as expired and lock (fail safe) |
| 9 | `lock_app_after` very large | Aniyomi computes `lockDelay * 60_000` in 32-bit `Int` (overflow above 35 791 minutes). Rust uses `i64`/`u64` arithmetic |
| 10 | Authentication unsupported or no unlock method configured | `use_authenticator` is set to false silently (Aniyomi `setAppLock`) |
| 11 | Biometric/unlock dialog on screen while the window deactivates | `isAuthenticating` true: `onStop` does nothing and `onStart` does not recompute |

### 1.4 Data conversions
| # | Situation | Behaviour |
|---|---|---|
| 1 | Genre containing `", "` | Splits into two genres on read; reproduced, not fixed (compat with backups) |
| 2 | `genre` is the empty string | Empty list (not `[""]`) |
| 3 | `update_strategy` = 7 (out of range) | `ALWAYS_UPDATE` |
| 4 | `fetch_type` = -1 | `Episodes` |
| 5 | `memo` BLOB not valid JSON | `FromSql` returns `Err(FromSqlError::Other)` (Kotlin throws); surfaces as a DB error to the caller |
| 6 | `last_read` = 0 | `Date(0)` = 1970-01-01; callers treat 0 as "never" (area:08) |
| 7 | Device texture limit is not a multiple of 1024 (for example 5000) | List `[5000, 4096, 3072, 2048]` |
| 8 | `pref_hardware_bitmap_threshold` stored below 2048 | Accepted as is (no clamp in Aniyomi) |
| 9 | `AppInfo` MIME list when the AVIF decoder feature is off | `image/avif` omitted; order of the rest unchanged |

### 1.5 Races
| # | Race | Resolution |
|---|---|---|
| 1 | Two threads call `ctx.manga_db()` first at the same time | `OnceLock::get_or_init` runs the closure once; the loser blocks until done |
| 2 | Warm-up opens the DB while the library screen also asks for it | Same as above; the second caller waits on the first |
| 3 | Migration and first UI query | Migrations complete before `build_ui` (blocking), so no overlap |
| 4 | Preference written from a tokio task while the GTK thread reads the binding | Store is `Send + Sync`; `watch` delivers to the GTK thread via `spawn_local`; stale reads last at most one main-loop iteration |
| 5 | Panic on worker thread while the main thread builds a window | Hook exits the process; the half-built window disappears |
| 6 | Crash dump read while the logger is still flushing | The file appender flushes on drop; the hook calls `tracing_appender` guard flush (`WorkerGuard` kept in a static) before spawning the reporter, bounded by 200 ms |
| 7 | Reporter started before the parent exits holds the D-Bus name | Reporter uses a different application id and `NON_UNIQUE`; "Restart" waits for the parent PID to be gone (poll `/proc/<pid>` for up to 2 s) before spawning the main app |

## 2. Upstream tests to port

Only one test file exists in or next to this scope: `app/src/test/java/mihon/core/migration/MigratorTest.kt` (JUnit 5, MockK, coroutines test). The Rust port goes into `crates/kuro-core/src/migrate/tests.rs` as plain `#[test]` functions with a recording closure instead of MockK spies; `tokio::test` is not needed because the pure functions `select` and `plan` are synchronous and `run` is awaited with `futures::executor::block_on`.

| Kotlin test | Rust test | Setup | Assertions |
|---|---|---|---|
| `initialVersion` | `initial_version_runs_only_always` | `select(0, 1)`; list `[Always -> true, At(2) -> false]` | strategy is `Initial`; `plan` returns exactly one migration (the Always one); after `run`, `on_complete` called once |
| `sameVersion` | `same_version_is_noop` | `select(1, 1)`; same list | strategy is `Noop`; `plan` is empty; `run` returns `Ok(false)`; `on_complete` never called; no migration executed |
| `noMigrations` | `range_with_empty_list` | `select(1, 2)`; empty list | strategy is `Range{from:2,to:2}`; result `Ok(false)`; no execution; `on_complete` never called |
| `smallMigration` | `small_range_includes_always_and_target` | `select(1, 2)`; `[Always, At(2)]` all true | `plan` length 2; `on_complete` called once |
| `largeMigration` | `large_range` | `select(1, 10)`; `[Always, At(2), At(3), At(4), At(5), At(6), At(7), At(8), At(9), At(10)]` | `plan` length 10; `on_complete` once; execution order is Always, 2, 3, 4, 5, 6, 7, 8, 9, 10 |
| `withinRangeMigration` | `range_excludes_future_versions` | `select(1, 2)`; `[Always, At(2), At(3)]` | `plan` length 2 (the `At(3)` is excluded); `on_complete` once |

The Kotlin test also configures `Dispatchers.setMain` with a single thread; nothing equivalent is needed.

## 3. New tests to write

### 3.1 `kuro-core::util::collections`
1. Empty: `insert_separators(vec![], g)` returns empty and `g` is never called (both functions).
2. Single item `["x"]`: generator calls recorded in order are `(None, Some("x"))`, `(Some("x"), None)` (two calls); reversed version records the same pair in the opposite order.
3. Grouped list `["a1","a2","b1"]` with generator "header when `before` is None or the first character differs, header text is `H:<first char of after>`, never emit when `after` is None": result `["H:a","a1","a2","H:b","b1"]`; forward call order `(None,a1) (a1,a2) (a2,b1) (b1,None)`; reversed call order `(b1,None) (a2,b1) (a1,a2) (None,a1)`; both produce the same output list.
4. Generator returning a separator on every call for `[1,2]` gives `[S0, 1, S1, 2, S2]` (length 5 = 2n+1).
5. Property test (proptest): for random vectors, the output with an always-`None` generator equals the input converted by `From`.

### 3.2 `kuro-core::db`
1. `configure` on a fresh temporary file: `PRAGMA foreign_keys` returns 1, `PRAGMA journal_mode` returns `wal`, `PRAGMA synchronous` returns 1 (NORMAL).
2. Foreign key enforcement: create parent/child tables, insert child with missing parent, expect constraint error (proves the pragma is per connection and applied to pooled connections).
3. `user_version` handling: new file gets `schema_version`; file with lower version calls `upgrade(from, to)` once inside a transaction; failed upgrade rolls back and `user_version` is unchanged; higher version returns `DbError::TooNew`.
4. Concurrent open from two threads against the same path does not corrupt or deadlock (busy timeout 5000 ms).

### 3.3 `kuro-core::db_types`
1. `StringList`: `["a","b"]` encodes to `a, b`; decode of `a, b` gives two items; decode of `""` gives empty; decode of `"x, y, z"` gives three; `["a, b"]` encodes to `a, b` and decodes to two items (documented sharp edge).
2. `UpdateStrategy`: ordinals 0 and 1 round-trip; 2, 99 and -1 decode to `ALWAYS_UPDATE`.
3. `FetchType`: 0 → `Seasons`, 1 → `Episodes`, 5 → `Episodes`.
4. `Memo`: `{"a":1}` encodes to the UTF-8 bytes `{"a":1}` (compact, key order as serde_json `Map` with `preserve_order` feature to keep insertion order); invalid bytes give an error.
5. `UnixMillis`: `1_700_000_000_000` round-trips; negative values accepted.

### 3.4 `kuro-core::crash`
1. Report format round trip: `build_report` for a synthetic panic then `parse_report` returns the same fields; unknown section preserved; unknown magic returns `Raw`.
2. Atomic write: killing the process between temp write and rename leaves only `.tmp` (simulated by injecting a failing rename) and the reporter ignores `.tmp` files.
3. `contain`: a panic inside `contain` returns `Err` and does not set `CRASHING`; a panic outside afterwards does.
4. Retention: with 25 files, 5 older than 14 days, startup cleanup leaves the newest 20 that are not older than 14 days.
5. End-to-end (integration test in the GUI crate, headless): run the binary with `KUROBAYOMI_CRASH_TEST=1`, `KUROBAYOMI_REPORTER_DRY=1` (test-only: the reporter prints the report path and exits 0 instead of opening a window) and `XDG_STATE_HOME=<tmp>`. Expect process exit code 101, one file `crashes/crash-*.txt`, first line `kurobayomi-crash-report 1`, a line `message: crash test`, and a `[backtrace]` section with at least one frame.
6. Worker variant: `KUROBAYOMI_CRASH_TEST=worker` panics in a tokio task; same assertions.

### 3.5 `kuro-core::crash_dump`
1. Golden file test of the debug block and extension block: input facts and a list of four `ExtensionStatus` values (one up to date, one with newer available version, one obsolete, one with no available entry and not obsolete) sorted by name; only the second and third appear; the missing available version prints `?`.
2. Log tail: a log file of 1000 lines of which 600 contain ` ERROR ` yields the last 500 error lines, in order, not exceeding 256 KiB; a missing file yields `(no log file)`.
3. File naming and replacement: dump twice; the second run deletes the first file first and the final file contains exactly one debug block.

### 3.6 `kuro-core::app_info`
1. `supported_image_mime_types()` default feature set returns exactly `["image/avif","image/gif","image/heif","image/jpeg","image/jxl","image/png","image/webp"]` in this order when all decoders are enabled; removing a decoder feature removes only its entry.
2. `version_name` has the `-<n>` suffix only when `build_type == Debug`.

### 3.7 `kuro-core::deep_link`
`parse("manga=12") == Some(Manga(12))`, `parse("manga") == None`, `parse("manga=abc") == None`, `parse("anime=0") == Some(Anime(0))`, `parse("updates")`, `parse("anime-extensions")` map to the variants of the table in `p1-data-and-config.md` section 7; unknown names return `None`; `name()` and `parse()` round-trip for every variant.

### 3.8 `theme_mode` and texture limits
1. Texture options: device 16384 returns the 15 values `16384, 15360, 14336, 13312, 12288, 11264, 10240, 9216, 8192, 7168, 6144, 5120, 4096, 3072, 2048`; device 8192 returns `8192, 7168, 6144, 5120, 4096, 3072, 2048`; device 4096 returns `4096, 3072, 2048`; device 3000 returns `3000, 2048`; device 2048 returns `2048`; every list starts with the device limit and is strictly decreasing.
2. `device_texture_limit()` honours `KUROBAYOMI_TEXTURE_LIMIT=4096`, ignores non-numeric values with a WARN, and never returns below 2048.
3. `theme_mode::apply(System)` after `apply(Dark)` restores the captured initial value (run under `xvfb-run`/headless Wayland in CI; skipped when no display).

### 3.9 App lock state machine (`kuro-core::applock` pure function or the area:36 service)
Table-driven over `(use_authenticator, is_authenticating, require_unlock, lock_after, last_closed, now) -> (require_unlock', last_closed')` for both transitions. Required rows:
| Transition | Inputs | Expected |
|---|---|---|
| stop | use_auth=false | no change |
| stop | use_auth, authenticating | no change |
| stop | use_auth, require_unlock=true | no change |
| stop | use_auth, require_unlock=false, lock_after=0 | no change (no timestamp) |
| stop | use_auth, require_unlock=false, lock_after=-1 | no change |
| stop | use_auth, require_unlock=false, lock_after=5, now=T | last_closed=T |
| start | use_auth, require_unlock=false, lock_after=-1 | require_unlock=false; last_closed deleted |
| start | same, lock_after=0 | require_unlock=true |
| start | same, lock_after=5, last_closed=T, now=T+299_999 | require_unlock=false |
| start | same, lock_after=5, last_closed=T, now=T+300_000 | require_unlock=true |
| start | same, lock_after=5, last_closed=T, now=T-1 (clock moved back) | require_unlock=true (fail safe deviation) |
| start | require_unlock already true | stays true; last_closed deleted |
| start | authenticating | require_unlock unchanged; last_closed deleted |

### 3.10 `PrefBinding` and `ProcessLifecycle` (GUI crate, display required)
1. `PrefBinding::get` equals the stored value after construction; `set(v)` does not change `get()` until the notification is processed (iterate the main context once); after iteration `get() == v`.
2. `bind` calls the closure immediately and once per change; three rapid sets yield at most three calls and the last call carries the last value.
3. `ProcessLifecycle`: activate → `Started`; deactivate then re-activate within 300 ms → no `Stopped`; deactivate for 800 ms → exactly one `Stopped`.

### 3.11 `AppContext`
1. `fresh_context_any_accessor_first`: for each accessor, build a fresh context and call only that accessor; none panics or deadlocks (run under a 10 s timeout).
2. `lazy_singleton_identity`: two calls return the same `Arc` (pointer equality).
3. `concurrent_first_access`: 8 threads released by a `Barrier` call `network()`; the constructor counter equals 1.

### 3.12 `kuro-testkit::FakeTracker`
Defaults equal `DummyTracker`: `logo_color = rgb(18,25,35)`, statuses `[1,2,3,4,5,6]`, completion status `2`, score list `["0".."10"]`, 10-point score `5.4`, username `"username"`, password `"passw0rd"`, not logged in; `client()` returns an error value instead of panicking.

## 4. Fixtures
* `fixtures/settings/valid.toml`, `fixtures/settings/corrupt.toml` (truncated mid-line), `fixtures/settings/unknown_keys.toml`.
* `fixtures/db/v0_empty.db` (created by the test), `fixtures/db/too_new.db` (`PRAGMA user_version = 9999`).
* `fixtures/crash/valid.txt`, `fixtures/crash/unknown_magic.txt`, `fixtures/crash/huge.txt` (generated, 2 MiB).
* `fixtures/logs/mixed.log` generated with 1000 lines, 600 of level ERROR.
* `fixtures/os-release/cachyos` and `fixtures/os-release/missing` for the debug-info test (`Paths`-style injection, the function takes the file contents).
* Test helper `TempPaths` implementing `Paths` over a `tempfile::TempDir` (in `kuro-testkit`), and `InMemorySettings` (the Rust analogue of `InMemoryPreferenceStore`).

## 5. Manual checks (release checklist for this area)
1. `KUROBAYOMI_CRASH_TEST=1 cargo run`: crash window appears within 2 s; both buttons work; "Restart" brings the app back.
2. Start with `XDG_CONFIG_HOME` pointing to an empty directory: app starts, `settings.toml` is created lazily on first change.
3. Corrupt `settings.toml`: app starts with defaults and a `.bad-` file appears.
4. Toggle incognito from the More screen: banner and notification appear/disappear within one frame / 1 s.
5. `kurobayomi --version` prints the build identity line and exits 0 without opening a window.
