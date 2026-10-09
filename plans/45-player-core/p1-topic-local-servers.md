# Topic: local servers (source HTTP server and torrent engine)

Scope files: `app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt` (122 lines), `app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt` (188 lines). Related but outside scope: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/HttpServer.kt`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt`, `core/common/src/main/java/aniyomi/core/common/torrent/*`. Behaviour: `p1-behavior.md` sections 17-19. Data: `p1-data-and-config.md` sections 2.4, 2.5, 3.7, 6.

## 1. Why these servers exist

Two cases need a server between mpv (or an external player) and the content:

1. Source video that the extension can only serve through its own HTTP handler (`AnimeHttpSource.createHttpServer()`), for example streams that need a per-request header or a decryption step. The app exposes the handler on `http://localhost:<port>`.
2. Torrent playback: the TorrServer engine streams torrent data over HTTP on `http://127.0.0.1:<port>`.

In both cases the video URL is rewritten before loading (`Video.copyHttpServer`, behaviour 3.7; torrent play link, behaviour 19.6).

## 2. Local source server (HttpServerService)

### 2.1 Android behaviour (OBSERVED)

- An Android foreground service with a `dataSync` type. `onStartCommand` stops any running server, creates one through the source, starts it, and shows a notification with a `Close` action (behaviour 17.2 and 18).
- `HttpServer` is NanoHTTPD bound to an ephemeral port (`NanoHTTPD(0)`). Its `start()` swallows bind errors (defect, behaviour 17.1).
- The UI waits up to 5 s for the running flag (`MainActivity.startHttpServerService`, behaviour 17.3).
- Static fields `_isRunning` and `port` are the only way the caller learns the port.

### 2.2 Rust design (Proposal, T-45a-069 to T-45a-071, T-45a-080)

```rust
pub struct LocalServer { /* tokio task, shutdown channel, bound port */ }

impl LocalServer {
    pub async fn start(handler: Arc<dyn StreamHandler>) -> Result<Self, std::io::Error>; // binds 127.0.0.1:0
    pub fn port(&self) -> u16;
    pub fn url(&self) -> String;             // "http://localhost:<port>"
    pub fn stop(self);
}
```

- Bind address: `127.0.0.1` only. NanoHTTPD binds all interfaces by default; the Rust server must not expose the source to the LAN.
- The URL uses `localhost` to match the rewrite rule (`localUrl` regex `http:\/\/localhost:1(?!\d)` and `copyHttpServer` output). The regex stays the same.
- Lifetime: owned by the player window (stop on window close) or by the external session (stop on external player exit). No foreground service.
- Start timeout: 5 s from the caller side (`tokio::time::timeout`), same as Kotlin.
- Restarting creates a new port. Any URL already handed to mpv becomes invalid (EC-112).
- Errors: bind failure is `Err` (defect fixed). The caller shows `Failed to start http server` (`AYMR.strings.http_server_start_failure`).

### 2.3 Byte-range streaming

The handler must support `Range` requests (mpv seeks with them). `hyper` 1.x with `http-body-util` gives streaming bodies. Test: `range_request_served` (`p1-edge-cases-and-tests.md` 5.7).

### 2.4 Notification (P2)

A desktop notification with a `Close` button (`gio::Notification` with an action, T-45a-071) replaces the Android ongoing notification. Channel and id constants (`http_server_channel`, `-901`) are dropped. Message: `Http server is running` (key `http_server_is_running`). Action label `Close` (key `action_close`).

## 3. Torrent engine (TorrentServerService)

### 3.1 Android behaviour (OBSERVED)

- Actions `start_torrent_server` and `stop_torrent_server` (`TorrentServerService.kt:141-142`).
- Start: if `echo` is empty, start the TorrServer library on `pref_torrserver_port` (default `8090`) with the data directory, proxy mode and proxy URL, wait up to about 11 s for `echo`, then push the tracker list (behaviour 19.2).
- Stop: stop the library, cancel the notification, stop the service.
- HTTP API used by the app (`TorrentServerApi.kt`): `GET /echo`, `POST /torrents` (JSON `add`), `POST /torrent/upload` (multipart). Play link: `GET /stream/<name>?link=<hash>&index=<n>&play`.
- Library calls: `TorrServer.startServer(port, path, proxyMode, proxyUrl): Int` (`-1` on failure), `stopServer()`, `addTrackers(String)`, `registerLogCallback()` (`TorrentServerService.kt:67-80`, `TorrentServerUtils.kt:18`).

### 3.2 Engine decision (open, recommendation below)

The Kotlin app uses TorrServer (Go, `io.github.secozzi:torrserver:0.1.1`). The Rust port has two options behind the `TorrentEngine` trait (`p1-rust-design.md` section 3.8):

| Option | How | Pros | Cons |
|---|---|---|---|
| A. TorrServer process | spawn a bundled or user-installed `torrserver` binary; talk to its HTTP API exactly as the Kotlin code does | same API, same play link, same tracker behaviour | Go binary to ship or install; process lifecycle to manage; port 8090 default |
| B. librqbit (pure Rust) | in-process BitTorrent client; expose our own `/stream/...` route through `LocalServer` | no external process; one stack; same play-link shape possible | must reimplement `/echo`, add/upload and the stream range behaviour; tracker list handled by the client API |

Recommendation (proposal): option B for the default path, option A kept as a fallback for users who already run TorrServer. Decision is needed before T-45a-073 is implemented; both must produce the same `play_link` shape (`p1-data-and-config.md` section 6.1) so the player code does not change.

Until the decision is made, the tasks T-45a-072 to T-45a-079 are P2 (not needed for daily reading and watching from one working source).

### 3.3 Torrent-specific rules (behaviour 19, data doc 3.7)

- Routing (T-45a-078): a video goes to the torrent path when support is enabled and its URL starts with the engine host, starts with `magnet`, or ends with `torrent`.
- `content://` torrent files (Android only): on Linux a local `.torrent` path is uploaded (T-45a-074).
- Magnet `index=<n>` picks the file (default 0).
- Play link: basename of the chosen file (or torrent title when the file is not found), URL-encoded UTF-8, `link` = info hash, `index` = file index, `play` flag.
- Trackers: stored as one URL per line (26 defaults, `p1-data-and-config.md` section 5), pushed as `,\n`-joined string.
- Proxy: `None` sends no URL; other modes send the URL with the value mapping `tracker`, `peers`, `full` (T-45a-077).

### 3.4 Lifecycle (Rust, proposal)

- One engine per process, owned by app state; started on first torrent playback; stopped on app exit (T-45a-080).
- A mutex prevents two concurrent starts (RC-09).
- `start` returns after `echo` answers or the 11 s budget elapses (EC-118: fail the start, do not continue with a dead port).
- Stop cancels the wait loop (RC-08).

### 3.5 Notification (P2)

Desktop notification `Torrent Server is running` (key `torrentserver_is_running`) with a `Stop` action (T-45a-079). The Kotlin label `Stop` is hard-coded English; the Rust label uses the existing MR key `action_stop` (value `Stop`, `i18n/src/commonMain/moko-resources/base/strings.xml`).

## 4. Security notes

- Both servers bind to loopback only. No authentication is added, because the only client is the local player.
- The local source server exposes the source's handler output to any local process that knows the port. Proposal: random port (already ephemeral) and a per-session token in the URL path (`/<token>/...`) that the rewrite inserts. This is a P2 hardening item and is not in the task list; mention in the parity doc if adopted.
- Torrent data is written under the app data directory only.

## 5. Failure modes

| Failure | Behaviour |
|---|---|
| Local server bind fails | error toast, no load (EC-113) |
| Source has no `AnimeHttpSource` | no server, error path via the source check (EC-111) |
| Server stopped mid-stream | mpv reports `HTTP error`; end-file error shows it (EC-114) |
| Torrent engine start fails | error toast `Failed to start torrent server` (proposal text), no load (EC-117) |
| Torrent API non-2xx | error toast with status (T-45a-073) |
| Magnet without `index=` | index 0 (EC-119) |
| Torrent with no hash | error, no link (EC-124) |

## 6. Tests

- Local server: `p1-edge-cases-and-tests.md` 5.7.
- Torrent play link, magnet index, trackers, proxy, wait budget: 5.8.
- Upstream fixtures for play-link tests: `TorrentHelpersTest` fixtures (section 1 of the edge-case document).
- Manual: section 7 items 10 and 11.
