# Topic — TorrServer lifecycle on Linux (replacing the Android AAR and foreground service)

Scope: `core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerApi.kt`, `TorrentServerUtils.kt`, `DisabledTorrServerException.kt`, plus the Android service that owns the server (`app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt`) and the playback caller (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:1093-1197`). Tasks: T-13a-040, T-13a-041, T-13a-044.

## 1. What the Android app does

- The server is a Go program (TorrServer, by the upstream project) compiled to an Android library with gomobile: `io.github.secozzi:torrserver:0.1.1` (`gradle/aniyomi.versions.toml:9,19`), imported as `xyz.secozzi.torrserver.TorrServer`. It runs **inside the app process**. Its Kotlin entry points, as called in this repository:
  - `TorrServer.startServer(port: String, path: String, proxyMode: String, proxyUrl: String): Int` (returns the bound port or `-1`, `TorrentServerService.kt:71-76`).
  - `TorrServer.stopServer()` (`:88`).
  - `TorrServer.addTrackers(trackers: String)` (`TorrentServerUtils.kt:18`).
  - `TorrServer.registerLogCallback()` (`:67`, verbose logging only).
- The app talks to it over HTTP on `127.0.0.1:<port>` (`TorrentServerApi.kt:22-23`).
- A foreground service (`TorrentServerService`) keeps the process alive, shows the ongoing notification, and is started by playback (`PlayerActivity.kt:1101`) and by the settings toggle.

## 2. Why the AAR cannot be reused

- The AAR is an Android artefact (JNI and gomobile bindings for Android). It cannot be linked into a Rust binary without re-binding the Go code.
- Its source is not in this repository, so its exact behaviour (tracker handling, port `0`, proxy mode strings) cannot be verified here. Open questions OQ-13a-1 to OQ-13a-3 depend on it.

## 3. Options

| Option | Description | Pros | Cons | Decision |
|---|---|---|---|---|
| A. Child process | Run a TorrServer-compatible binary (`torrserver`) as a child, talk HTTP | No Go toolchain in the Rust build; process isolation; same HTTP API as the Android app | The binary must be obtained and versioned; flags and environment differ from the AAR calls | **Chosen** (proposal) |
| B. Rebuild the Go core into a C archive and link through cgo | Use the upstream TorrServer Go source, `go build -buildmode=c-archive` | Same code, in process | Adds Go to the build, large binary, complex cross-compile, and Windows later needs the same work | Rejected for now |
| C. Drop torrent playback on Linux | No torrent streaming | Simplest | Loss of a whole anime feature (torrent sources) | Rejected unless A fails in the spike |

Decision (proposal, to be confirmed by the spike in T-13a-041): option A. The spike verifies that a TorrServer build exists for Linux x86_64 with the endpoints in p1-data-and-config.md section 5, and records its version string in the release notes.

## 4. Supervisor design (option A)

### 4.1 Process and binary discovery

1. Binary path, in order: (a) user setting `torrserver_binary` (a path, empty by default, not in the torrent preference list of Android: a new key, `pref_torrserver_binary`, to be added in T-13a-041); (b) `torrserver` on `PATH` (resolved through kuro-platform); (c) bundled path under the app data directory (`<data>/bin/torrserver`), installed by the packaging step, not downloaded at runtime.
2. If none is found, `ServerStatus::Failed("TorrServer binary not found")`; the settings row shows the reason and a link to the setup note (no download by the app).
3. The child is started with: `--port <port>` (the configured port, `0` allowed when the server reports the bound port), `--data <data>/torrserver`, and environment variables for the proxy: `PROXY_MODE` and `PROXY_URL` (names to be confirmed against the binary; if the binary takes flags instead, use flags). Standard output and error go to the app log (`tracing` target `torrserver`).
4. `kill_on_drop(true)` on the `tokio::process::Child`; the supervisor task owns it.

### 4.2 State machine

```
Stopped --ensure_running--> Starting --probe ok (echo non-empty)--> Running{port}
Starting --child exits / probe timeout (11 polls)--> Failed(reason)
Running --stop()--> Stopping --child exited--> Stopped
Running --child exited unexpectedly--> Failed(reason)
Failed --ensure_running--> Starting
Stopped --stop()--> Stopped
Running --ensure_running()--> Running (no-op, returns port)
Starting --ensure_running()--> waits on the same start (single flight)
```

- `ensure_running` is single-flight: the first caller creates a `tokio::sync::watch` for the start result; concurrent callers await it.
- Readiness probe: `TorrServerClient::echo()` once per second, at most 11 probes, mirroring `wait(10)` (`TorrentServerService.kt:79,173-186`). After the probe succeeds: push trackers (section 5), then `Running{port}`.
- The port written to `TorrServerClient` is the port the server reports (for configured `0`, the bound port), never the configured `0`.

### 4.3 Stop

- `stop()`: send SIGTERM to the child, wait up to 5 s, then SIGKILL; set `Stopped`. Matches the Kotlin `stopServer()` plus `stopSelf()`.
- Disabling the torrent switch calls `stop()` (`PlayerSettingsTorrentScreen.kt:84-86`). Changing trackers or proxy calls `stop()` (`:120-122`); the next playback restarts it.
- App shutdown: stop, then drain (T-13a-048).

### 4.4 Failure surfaces

| Failure | Status | Message key shown | User action |
|---|---|---|---|
| Binary missing | `Failed` | "TorrServer is not running" in toast; reason in settings | install binary |
| Port in use | `Failed("port N in use")` (detected by the child's exit code or probe on a foreign process) | reason | change port |
| Probe timeout | `Failed("did not respond within 11 s")` | reason | retry |
| Child crash | `Failed("exited with code X")` | reason | retry |

Desktop notification only on the first transition to `Failed` (not on user stop).

## 5. Trackers

- Android: `TorrServer.addTrackers(animeTrackers)` once after start, with the list from the preference captured at singleton construction (defect D-12: stale until restart).
- Linux options (decision after OQ-13a-1):
  - (a) A settings endpoint on TorrServer (if the chosen build exposes one, verify the request shape against its API docs): push the list after each start. Preferred if it exists.
  - (b) Append the trackers to each magnet link at add time (`&tr=<url-encoded>` for each tracker). Works with any TorrServer version that accepts magnets. Applied in `TorrServerClient::add` only for magnet links. Cost: longer URLs; no effect on `.torrent` uploads (their trackers are in the file).
- Both: read the preference at each start, trim each line, drop empty lines, and keep the server-side list in the same order.

## 6. Proxy

- Proxy mode and URL are passed at start (section 4.1). Changing them stops the server (settings rule).
- `ProxyMode::None` maps to "no proxy" (OQ-13a-3; deviation D-13). `Tracker`, `Peers`, `Full` map to the server's modes `tracker`, `peers`, `full` as in `TorrentPreferences.kt:46-51`.
- The URL is validated before start (`validate_proxy_url`, T-13a-043). An invalid stored URL with mode not `None` causes `Failed("invalid proxy url")` without starting the child.

## 7. Playback routing (PlayerActivity)

Observed (`PlayerActivity.kt:1093-1197`), kept as-is in `route.rs`:

```
if torrents enabled AND (url starts with server_base OR url starts with "magnet" OR url ends with "torrent"):
    ensure_running()  // may fail -> toast
    if url starts with "content://":      // local .torrent
        upload bytes (save=false), play link index 0
    elif url starts with "magnet":
        index = magnet_index(url)          // default 0
        add(url, title, save=false), play link (index)
    else:
        add(url, title, save=false), play link index 0   // TorrServer fetches the .torrent itself
else:
    normal playback
```

Open point (OQ-13a-2): the play link index is 0-based in the app (default `0`), while `FileStats.id` is 1-based for multi-file torrents. Test against the chosen TorrServer build before shipping; the answer goes in T-13a-044's commit message.

Errors: `ensure_running` failure shows the toast and stops; `add`/`upload` failure shows the toast with the mapped key.

## 8. Notification and status (desktop)

- No ongoing system notification on Linux. The status is the settings row (p1-ui-parity.md section 3).
- A desktop notification only on `Failed` (through `kuro-platform::Notifier`).

## 9. Timings and limits (collected)

| Item | Value |
|---|---|
| readiness probe interval | 1 s |
| readiness probe count | 11 (initial `wait(10)`) |
| stop grace period | 5 s, then kill (proposal) |
| HTTP timeout for `add` | 30 s (proposal; the Kotlin uses the app HTTP client default, area 12) |
| HTTP timeout for `echo` | 2 s (proposal) |

## 10. Test plan

- Unit: state machine transitions (table in section 4.2) with a fake child (a test binary that prints a port and serves `/echo`).
- Integration: start the real binary (if present in CI) and run `add`/`stream_url` against a fixture torrent.
- Race: 20 concurrent `ensure_running` calls start one child.
- Port `0`: the bound port is used.
- Stop during start: no orphan child (check with `pgrep` in the test).

## 11. Risks

- The TorrServer binary is an external dependency with its own licence and update cadence; the packaging decision belongs to the release process (not this area).
- Running a peer-to-peer server exposes the user's IP to peers (the legal notice in p1-ui-parity.md section 2.2 covers it; the Linux port keeps the notice).
- Loopback-only binding is required (`127.0.0.1`); the supervisor passes the bind address explicitly and a test checks that no `0.0.0.0` listener exists.
