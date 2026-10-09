# 24-downloads-anime: Rust design

Target: Kurobayomi (`/home/blackbox/kurobayomi`). Existing manifest: `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, `serde`, `serde_json`, edition 2024. The mockup has no async runtime yet. This design adds one. Behaviour references point to `p1-behavior.md` sections (written "behaviour §n"). Source paths are repo-relative to `/home/blackbox/aniyomi-src`.

---

## 1. Module layout

New code goes in `src/downloads/` (engine, no GTK types) and `src/downloads_page.rs` (GTK page). Nothing in `src/downloads/` may import `gtk`.

```
src/downloads/
  mod.rs            public facade: DownloadManager (behaviour §2, §4, §6 API)
  model.rs          Download, DlState, EpisodeRef, AnimeRef, SourceRef            (behaviour §1)
  error.rs          DlError (user-facing messages)                                 (behaviour §14)
  store.rs          SQLite queue store: anime_download_queue                      (data §5.1)
  pending.rs        SQLite pending-delete store: anime_download_pending_delete     (behaviour §7, data §5.2)
  layout.rs         directory naming and lookups                                  (behaviour §9)
  cache.rs          downloaded-episode index, renewal, snapshot                   (behaviour §8)
  scheduler.rs      per-source queue scheduling                                   (behaviour §2.3)
  engine.rs         per-download pipeline: resolve, stage, download, finalise     (behaviour §5)
  retry.rs          retry schedule 2/4/8 s, 4 attempts                            (behaviour §5.5)
  ffmpeg.rs         argv builder, ffprobe, progress parser, child process driver  (behaviour §5.7)
  torrent.rs        torrent-server path (feature-gated, see T-24a-023)           (behaviour §5.6)
  external.rs       external-handoff (feature-gated, see T-24a-024)              (behaviour §5.4)
  lifecycle.rs      runner state, network gating, start/stop                     (behaviour §4.2)
  network.rs        NetworkManager state (zbus) + sysfs fallback                 (behaviour §12)
  notify.rs         progress/warning/error notifications                         (behaviour §10)
src/downloads_page.rs   GTK page: source groups, rows, menus, DnD, FAB            (ui-parity)
```

Keep `pending.rs` as one file. Keep `external.rs` and `torrent.rs` out of the first build (they are P2/P3 tasks) but leave the enum variants and trait hooks in place so they do not change the public API later.

---

## 2. Crates

| Need | Crate | Why / replaces |
|---|---|---|
| Async runtime for scheduler, timers, child processes | `tokio` with features `rt-multi-thread`, `sync`, `process`, `time`, `fs`, `macros` | Replaces Kotlin coroutines. Runs on a dedicated background thread, separate from the GTK main loop. |
| Cancellation per download | `tokio-util` (`CancellationToken`) | Replaces `Job.cancel()` and `invokeOnCancellation`. |
| `BoxFuture` for `dyn` traits | `futures` (`future::BoxFuture`) | Avoids `async_trait`. |
| State fan-out (status, progress) | `tokio::sync::watch` | Replaces `MutableStateFlow`. Runtime-agnostic, so it can be awaited on the GTK thread too. |
| Queue change wake-up | `tokio::sync::Notify` | Replaces the queue `StateFlow` re-trigger in the scheduler. |
| Bridge to GTK | `async-channel` + `glib::spawn_future_local` | Engine events arrive on the GTK main context without blocking it. |
| Database | `rusqlite` (`bundled` feature recommended) | Replaces SharedPreferences for queue and pending deletes. Bundled avoids depending on the system SQLite version. |
| Cache snapshot | `bincode` (2.x, serde feature) plus `serde` (already present) | Replaces kotlinx ProtoBuf. |
| Paths | `directories` (XDG) | Replaces `context.cacheDir`. |
| ffmpeg / ffprobe | `std::process` via `tokio::process::Command` (binaries on PATH) | Replaces FFmpegKit. No crate: the binary is the contract. |
| Free space | `rustix` feature `fs` (`statvfs`) | Replaces `StatFs`. |
| D-Bus: NetworkManager | `zbus` | Replaces `ConnectivityManager`. |
| Desktop notifications | `notify-rust` | Replaces `NotificationCompat`. Uses `org.freedesktop.Notifications`, supports actions and replace-id. |
| Logging | `tracing` + `tracing-subscriber` | Replaces `logcat`. |
| Errors | `thiserror` | `DlError`. |
| Clipboard | `gtk::gdk::Clipboard` (already in gtk) | Replaces `copyToClipboard`. |

Do not add `anyhow` to this module. Errors cross the GTK boundary as `DlError` and are displayed with their `Display` text, matching Kotlin's `Throwable.message`.

Version policy: pin exact versions in `Cargo.toml` when adding. Match the `glib` version pulled by `gtk4 0.11.5` for any direct `glib` use.

---

## 3. Threading and async model

Two worlds:

1. **GTK main thread** (`glib::MainContext`): all widgets, all `gio::ListStore` / ListBox mutation, all menu actions.
2. **Engine runtime** (`tokio` multi-thread runtime, 2 worker threads, started once in `main`): scheduler, per-download tasks, ffmpeg children, network watcher, notification sends, DB writes (through `spawn_blocking`).

Bridging rules:
- Commands from GTK to engine: `DownloadManager` methods are `async fn` or `fn` that push onto an `mpsc::UnboundedSender<Command>`. The GTK side never awaits the engine.
- Events from engine to GTK: every `Download` has `watch::Receiver<DlState>` and `watch::Receiver<i32>` (progress). The page subscribes once per row (see §10) using `glib::spawn_future_local`.
- Queue snapshot: `watch::Sender<Arc<Vec<Arc<Download>>>>` published after every mutation. GTK rebuilds its groups from the snapshot. This replaces `queueState` plus `statusFlow` and `progressFlow`.
- Never call a blocking DB function from the GTK thread. `startDownloadNow` in Kotlin uses `runBlocking`; in Rust the GTK side sends `Command::StartNow { episode_id }`, and the engine performs the lookup.

Single-writer rule for the queue: only the scheduler task owns the mutable queue `Vec`. Other tasks send commands. This replaces the Kotlin "queue manipulation must be done in one thread" comment (`DL/AnimeDownloader.kt:80`) with an enforced structure.

```rust
// src/downloads/mod.rs
pub struct DownloadManager {
    cmd: tokio::sync::mpsc::UnboundedSender<Command>,
    queue: tokio::sync::watch::Receiver<Arc<Vec<Arc<Download>>>>,
    runner: tokio::sync::watch::Receiver<RunnerState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerState { Stopped, Running, Paused }

pub enum Command {
    Enqueue { anime: AnimeRef, episodes: Vec<EpisodeRef>, auto_start: bool, alt: bool, video: Option<Video> },
    AddToStart(Vec<Arc<Download>>),
    StartNow { episode_id: i64 },
    Reorder(Vec<i64>),                       // episode ids in new order
    Cancel { episode_ids: Vec<i64> },
    CancelAnime { anime_id: i64 },
    Start,
    Pause,
    ClearQueue,
    DeleteEpisodes { anime: AnimeRef, episode_ids: Vec<i64> },
    DeleteAnime { anime: AnimeRef, remove_queued: bool },
    EnqueueDelete { anime: AnimeRef, episode_ids: Vec<i64> },
    FlushPendingDeletes,
    RenameEpisode { anime: AnimeRef, old: EpisodeRef, new: EpisodeRef },
    RenameSource { old_source_id: i64, new_source_id: i64 },
}
```

Note the reorder payload carries ids, not `Arc<Download>`, so the engine re-resolves them. That matches `updateQueue` semantics (`DL/AnimeDownloader.kt:918-936`): items are kept, statuses reset to QUEUE, store rewritten.

---

## 4. Core types

```rust
// src/downloads/model.rs
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DlState {
    NotDownloaded = 0,
    Queue = 1,
    Downloading = 2,
    Downloaded = 3,
    Error = 4,
}

impl DlState {
    /// Behaviour §1: "active" means value <= Downloading.
    pub fn is_active(self) -> bool { (self as i32) <= (DlState::Downloading as i32) }
}

/// Identity of a queued download is the episode id. Never derive Hash/Eq from `video`.
pub struct Download {
    pub episode: EpisodeRef,
    pub anime: AnimeRef,
    pub source: SourceRef,
    pub change_downloader: bool,
    pub video: Mutex<Option<Video>>,              // mutable; mirrors `var video`
    pub state: watch::Sender<DlState>,
    pub progress: watch::Sender<i32>,             // 0..=100; 0 means indeterminate in the UI
    pub order_hint: std::sync::atomic::AtomicI64, // queue order cache, not persisted
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpisodeRef {
    pub id: i64,
    pub anime_id: i64,
    pub url: String,
    pub name: String,
    pub scanlator: Option<String>,
    pub seen: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub source_order: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimeRef {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub source_id: i64,
}

/// Installed HTTP-capable source. Created by the source runtime (area owned elsewhere).
pub struct SourceRef {
    pub id: i64,
    pub name: String,
    pub lang: String,
    pub headers: Vec<(String, String)>,          // ordered header pairs (OkHttp Headers semantics)
    pub unmetered: bool,                          // maps UnmeteredSource
    pub is_http: bool,                            // false for local sources (queueEpisodes ignores them)
}

impl SourceRef {
    /// Behaviour §12 / data §4.1: "$name (${lang.uppercase()})"
    pub fn dir_name_source(&self) -> String { format!("{} ({})", self.name, self.lang.to_uppercase()) }
}
```

`Download` deliberately has no `PartialEq`/`Hash`. The Kotlin data class equality over a mutable `video` is a defect (behaviour §2, defect in T-24a-051). Any map keyed by a download uses `episode.id`.

Video model (engine-local, a subset of `Video` from area:16-source-api-anime):

```rust
#[derive(Clone, Debug, Default)]
pub struct Video {
    pub video_url: String,
    pub video_title: String,
    pub headers: Option<Vec<(String, String)>>,   // None means fall back to SourceRef.headers
    pub subtitle_tracks: Vec<Track>,
    pub audio_tracks: Vec<Track>,
    pub ffmpeg_stream_args: Vec<(String, String)>,
    pub ffmpeg_video_args: Vec<(String, String)>,
    pub initialized: bool,
    pub status: VideoStatus,
}

#[derive(Clone, Debug)]
pub struct Track { pub url: String, pub lang: String }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VideoStatus { #[default] Queue, LoadVideo, Ready, Error }
```

---

## 5. Traits to the outside world

These are the seams. Each one is implemented elsewhere in Kurobayomi; this area only defines the trait.

```rust
use futures::future::BoxFuture;

/// Resolves the best playable video for an episode (behaviour §5.2).
/// Maps EpisodeLoader.getHosters + HosterLoader.getBestVideo (area:45-player-core).
pub trait HosterResolver: Send + Sync {
    fn best_video<'a>(&'a self, anime: &'a AnimeRef, episode: &'a EpisodeRef, source: &'a SourceRef)
        -> BoxFuture<'a, Result<Option<Video>, DlError>>;
}

/// Lookup of installed sources (AnimeSourceManager.get / getOrStub / getAll / getStubSources).
pub trait SourceLookup: Send + Sync {
    fn get(&self, source_id: i64) -> Option<SourceRef>;
    fn get_or_stub(&self, source_id: i64) -> SourceRef;
    fn all_including_stubs(&self) -> Vec<SourceRef>;
    fn is_initialised(&self) -> watch::Receiver<bool>;
}

/// Category membership of an anime (GetAnimeCategories.await).
pub trait CategoryLookup: Send + Sync {
    fn category_ids_for_anime(&self, anime_id: i64) -> BoxFuture<'_, Result<Vec<i64>, DlError>>;
}

/// Episode and anime reads used by restore (GetEpisode/GetAnime).
pub trait EntityLookup: Send + Sync {
    fn episode(&self, id: i64) -> BoxFuture<'_, Result<Option<EpisodeRef>, DlError>>;
    fn anime(&self, id: i64) -> BoxFuture<'_, Result<Option<AnimeRef>, DlError>>;
}

/// Downloads root and filesystem roots (StorageManager + DiskUtil).
pub trait StorageRoot: Send + Sync {
    fn downloads_dir(&self) -> Option<std::path::PathBuf>;
    fn changes(&self) -> watch::Receiver<u64>;    // bumps when the root folder changes
}

/// Local HTTP server for `localhost:1xxxx` video URLs (behaviour §5.3, T-24a-022).
pub trait LocalHttpServer: Send + Sync {
    fn start(&self, source_id: i64) -> BoxFuture<'_, Result<u16, DlError>>; // returns listening port
    fn stop(&self, port: u16);
}
```

Injected in `DownloadManager::new(...)`. Nothing in `src/downloads/` names a concrete source, extension, or database type outside `store.rs` and `pending.rs`.

---

## 6. Scheduler (behaviour §2.3)

Algorithm in Rust (one task owns the queue):

```rust
// src/downloads/scheduler.rs
pub struct Scheduler {
    queue: Vec<Arc<Download>>,                 // single owner
    running: HashMap<i64, RunningDownload>,    // key = episode id
    runner: RunnerState,
    notify: Arc<tokio::sync::Notify>,
}

pub struct RunningDownload {
    pub cancel: tokio_util::sync::CancellationToken,
    pub handle: tokio::task::JoinHandle<()>,
    pub source_id: i64,
}

/// Behaviour §2.3: first download (in queue order) of each source that is active; at most 3 sources.
pub fn active_set(queue: &[Arc<Download>]) -> Vec<Arc<Download>> {
    let mut order: Vec<i64> = Vec::new();
    let mut firsts: HashMap<i64, Arc<Download>> = HashMap::new();
    for d in queue.iter().filter(|d| d.state.borrow().is_active()) {
        if !firsts.contains_key(&d.source.id) {
            order.push(d.source.id);
            firsts.insert(d.source.id, d.clone());
        }
    }
    order.into_iter().take(3).map(|id| firsts.remove(&id).unwrap()).collect()
}
```

Notes:
- `take(3)` is the constant. Do not use 5 (the Kotlin comment is wrong).
- Group key is `source.id`, which is stable. The Kotlin key is the `AnimeSource` object (`groupBy { it.source }`); equality for a source is by identity of the installed source, which maps to id.
- "Wait until any active item reaches ERROR" becomes a `FuturesUnordered` over each running item's `watch::Receiver<DlState>` plus `notify.notified()`. When any item becomes `Error`, the scheduler recomputes `active_set` and starts new work. Items that finish with `Downloaded` are removed from the queue, which also triggers recompute through `notify`.
- End of run: if `active_set` is empty and no item is active, call `finish_run()`: if the queue has no active item, transition to `Stopped` and emit `onComplete` (queue empty) or `onPaused` (queue has ERROR items) — keep the Kotlin decision, and see behaviour §2.5 defect. Proposal: emit `Finished { errors: n }` and let the notifier decide the text.

Per-download task (behaviour §5):

```rust
async fn run_download(ctx: EngineCtx, d: Arc<Download>, cancel: CancellationToken) {
    let result = tokio::select! {
        _ = cancel.cancelled() => { d.state.send_replace(DlState::Queue); return; }   // pause: back to QUEUE (behaviour §2.6)
        r = engine::download_episode(&ctx, &d, &cancel) => r,
    };
    match result {
        Ok(()) => { d.state.send_replace(DlState::Downloaded); ctx.remove_from_queue(d.episode.id).await; }
        Err(DlError::Cancelled) => {}  // state already set by the canceller
        Err(e) => { d.state.send_replace(DlState::Error); ctx.notify.error(&d, &e); }
    }
}
```

Important: per-episode failures do not stop the runner (behaviour §2.4 explains this). Only unexpected errors that escape `download_episode` (for example staging dir creation failure) call `ctx.stop_with_error(e)`, which is the equivalent of `launchDownloadJob`'s catch-all `stop()`.

Pause / cancel rules:
- `Pause`: cancel every running token. Each running item goes to `Queue` (behaviour §2.6). Partial staging output is removed by the download task (behaviour §5.5).
- `Cancel { ids }`: cancel the running token for those ids, remove them from the queue and the store, set state `NotDownloaded`. Behaviour §2.12.
- `ClearQueue`: cancel all, remove all, store cleared. `DOWNLOADING` and `QUEUE` become `NotDownloaded`; `ERROR` also disappears because the list is emptied. Behaviour §2.7.

---

## 7. Per-download pipeline (behaviour §5)

```rust
// src/downloads/engine.rs
pub async fn download_episode(ctx: &EngineCtx, d: &Arc<Download>, cancel: &CancellationToken) -> Result<(), DlError> {
    let anime_dir = layout::get_anime_dir(&ctx.root()?, &d.source, &d.anime.title)?;         // creates dirs; behaviour §9.3
    match free_space(&anime_dir) {                                                          // rustix::fs::statvfs
        Some(avail) if avail < MIN_DISK_SPACE => {
            d.state.send_replace(DlState::Error);
            return Err(DlError::LowSpace);                                                   // notify: download_insufficient_space
        }
        _ => {}
    }
    let episode_dirname = layout::episode_dir_name(&d.episode.name, d.episode.scanlator.as_deref());
    let tmp_dir = anime_dir.join(format!("{episode_dirname}{TMP_DIR_SUFFIX}"));
    std::fs::create_dir_all(&tmp_dir)?;                                                     // failure escapes -> stop_with_error

    let outcome = async {
        if d.video.lock().unwrap().is_none() {
            let v = ctx.hosters.best_video(&d.anime, &d.episode, &d.source).await?
                .ok_or(DlError::NoVideo)?;
            *d.video.lock().unwrap() = Some(v);
        }
        get_or_download_video_file(ctx, d, &tmp_dir, cancel).await?;
        if !is_download_successful(&tmp_dir)? { d.state.send_replace(DlState::Error); return Ok(()); }
        let filename = fat_safe(&format!("{} - {}", d.anime.title, d.episode.name));
        let _ = std::fs::remove_file(tmp_dir.join(format!("{filename}_tmp.mkv")));
        std::fs::rename(&tmp_dir, anime_dir.join(&episode_dirname))?;
        ctx.cache.add_episode(&episode_dirname, &anime_dir, &d.anime).await;
        layout::create_nomedia(&anime_dir.join(&episode_dirname))?;                        // fixed: create in FINAL dir (behaviour §5.9)
        d.state.send_replace(DlState::Downloaded);
        Ok(())
    }.await;
    outcome
}

pub const MIN_DISK_SPACE: u64 = 200 * 1024 * 1024;   // behaviour §13
pub const TMP_DIR_SUFFIX: &str = "_tmp";
```

Implementation rules:
- Use `std::fs::rename` for the directory rename. It is atomic on the same filesystem. If it fails with `EXDEV`, return an error. Do not copy across filesystems.
- `is_download_successful` checks exactly one regular entry in `tmp_dir` whose name does not end in `.tmp` (fixes the defect in behaviour §5.8).
- `create_nomedia` runs after the rename, on the final path. This fixes the observed stale-handle issue.
- The `Video` resolution step runs only when `video` is `None` (same as Kotlin). A resumed download with a stored `video` skips it.

Internal download (behaviour §5.3–§5.7):

```rust
async fn get_or_download_video_file(ctx: &EngineCtx, d: &Arc<Download>, tmp_dir: &Path, cancel: &CancellationToken) -> Result<PathBuf, DlError> {
    let filename = fat_safe(&d.episode.name);
    let _ = std::fs::remove_file(tmp_dir.join(format!("{filename}.tmp")));
    if let Some(existing) = find_finished_remux(tmp_dir, &filename) { return Ok(existing); }   // starts_with("$filename.mkv")

    d.state.send_replace(DlState::Downloading);
    d.progress.send_replace(0);
    let use_external = ctx.prefs.use_external_downloader() == d.change_downloader;
    if use_external { ctx.start_internal(d, tmp_dir, &filename, cancel).await }
    else            { ctx.start_external(d, tmp_dir, &filename, cancel).await }
}
```

- The progress poll loop from Kotlin (50 ms) is replaced by ffmpeg `-progress pipe:1` parsing. The engine publishes at most 10 Hz into `d.progress` (see §11). Do not poll.
- Errors from this function are returned as `Err`; the caller maps them to `Error` plus notification. This differs from the Kotlin code, which swallows and notifies inside. The user-visible result is the same, but the double notification (behaviour §5.3) disappears.

ffmpeg driver (behaviour §5.7):

```rust
// src/downloads/ffmpeg.rs
pub fn build_remux_args(video: &Video, source_headers: &[(String, String)], out: &Path) -> Vec<OsString>;
pub fn build_ffprobe_args(video_url: &str, header_arg: &str) -> Vec<OsString>;
pub fn parse_progress_line(line: &str) -> Option<ProgressTick>;

pub enum ProgressTick { OutTimeMicros(i64), Ended }
```

`build_remux_args` must produce exactly this order (behaviour §5.7.1), using separate argv elements:

```
[ -headers "<hdr>" ]? -progress pipe:1 -nostats -stats_period 0.5 <stream args...> -i <video_url>
[ -headers "<hdr>" ]? -i <subtitle_url>  (for each subtitle track)
[ -headers "<hdr>" ]? -i <audio_url>     (for each audio track)
-map 0:v
-map <i + 1 + subtitles_len>:a          (for each audio track)
-map 0:a?
-map <i + 1>:s                           (for each subtitle track)
-map 0:s? -map 0:t?
-f matroska -c:a copy -c:v copy -c:s copy
-metadata:s:s:<i> title=<lang>           (for each subtitle track)
-metadata:s:a:<i> title=<lang>           (for each audio track)
-<k> <v>                                  (ffmpeg_video_args)
<out> -y
```

Points:
- `-headers` argv element value is `"Name: value\r\n"` concatenated, which is exactly the Kotlin string. Include it before an input only when that input's URL starts with `http`. The video input header is also gated on `http`.
- Note the Kotlin order is: input-stream args before `-i`, so `ffmpeg_stream_args` come after `-headers` and before `-i`.
- The `-progress pipe:1 -nostats` and `-stats_period` additions are new, required for the progress parser. They do not change output.
- Output path `out` is passed as an argv element (no quoting). The Kotlin `-y` flag stays.
- The ffmpeg binary is found on `PATH` (`which ffmpeg` at startup). If missing, the Settings screen shows an error and downloads fail with `DlError::FfmpegMissing` (new message, see §12).

Progress parse: lines are `key=value`. Use `out_time_us=<microseconds>` (ffmpeg emits microseconds under this key; the Kotlin `s.time` is milliseconds, so the Rust port derives seconds as `out_time_us / 1_000_000`). When `progress=end` is seen, the run is finished.

Percent: `pct = min(100, 100 * out_seconds / duration_seconds)` where `duration_seconds` is the integer result of ffprobe (`toFloatOrNull()` then `toLong()`, behaviour §5.7). If `duration == 0` or `out_seconds == 0`, leave progress at 0 (indeterminate). Publish to `d.progress` only when the integer value changes.

Torrent (behaviour §5.6, `torrent.rs`): feature-gated behind `torrent-server`; the first build stubs it with `DlError::TorrentUnavailable`. Conversion logic (`link=` / `index=` parse, magnet build, play-link lookup) is described in behaviour §5.6 and must be a pure function with a unit test.

External (behaviour §5.4, `external.rs`): see T-24a-024. Default build omits it. Must not finalise on handoff.

---

## 8. Retry (behaviour §5.5)

```rust
// src/downloads/retry.rs
pub const RETRY_DELAYS_MS: [u64; 3] = [2_000, 4_000, 8_000];

pub async fn with_retry<F, Fut, T>(cancel: &CancellationToken, mut op: F) -> Result<T, DlError>
where F: FnMut() -> Fut, Fut: std::future::Future<Output = Result<T, DlError>>
{
    let mut attempt = 0usize;
    loop {
        match op().await {
            Ok(v) => return Ok(v),
            Err(DlError::Cancelled) => return Err(DlError::Cancelled),
            Err(e) if attempt < RETRY_DELAYS_MS.len() => {
                let delay = std::time::Duration::from_millis(RETRY_DELAYS_MS[attempt]);
                attempt += 1;
                tokio::select! {
                    _ = cancel.cancelled() => return Err(DlError::Cancelled),
                    _ = tokio::time::sleep(delay) => {}
                }
                let _ = e;   // logged at debug level
            }
            Err(e) => return Err(e),
        }
    }
}
```

This matches the Kotlin schedule exactly: 4 attempts total, delays 2 s, 4 s, 8 s. Each attempt starts by deleting the partial `.tmp` (behaviour §5.5), so `op` must include that step.

---

## 9. Persistence

```rust
// src/downloads/store.rs
pub struct QueueStore { conn: Arc<std::sync::Mutex<rusqlite::Connection>> }

impl QueueStore {
    pub fn add_all(&self, items: &[(i64 /*episode*/, i64 /*anime*/, i64 /*ord*/)]) -> rusqlite::Result<()>;
    pub fn remove(&self, episode_id: i64) -> rusqlite::Result<()>;
    pub fn remove_all(&self, episode_ids: &[i64]) -> rusqlite::Result<()>;
    pub fn clear(&self) -> rusqlite::Result<()>;
    pub fn restore(&self) -> rusqlite::Result<Vec<(i64, i64)>>;  // (anime_id, episode_id) ORDER BY ord
}
```

Rules:
- Writes happen after the in-memory change, inside the scheduler task, through `spawn_blocking`. Keep the in-memory queue as truth during a session.
- `restore` is called once at startup. Entries whose anime, episode or source is missing are dropped (behaviour §2.1). Proposal: keep rows whose anime exists but whose source is not installed yet, and mark them `Queue` but unstartable (fixes the observed loss; T-24a-009).
- `restore` then `clear` then `add_all` in one transaction, as in Kotlin, with fresh `ord` values.

```rust
// src/downloads/pending.rs
pub struct PendingDeleter { conn: Arc<std::sync::Mutex<rusqlite::Connection>> }

impl PendingDeleter {
    /// Dedupe by episode_id, against existing rows and within the batch (behaviour §7).
    pub fn add_episodes(&self, episodes: &[EpisodeRef], anime: &AnimeRef) -> rusqlite::Result<()>;
    /// Returns every row grouped by anime. Deletes only rows whose source resolved; rows with a missing source stay.
    pub fn take_for_installed(&self, installed: &dyn Fn(i64) -> bool) -> rusqlite::Result<Vec<(AnimeRef, Vec<EpisodeRef>)>>;
}
```

Cache snapshot (`cache.rs`):

```rust
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct CacheSnapshot {
    pub version: u32,                                  // 3 to match "dl_anime_index_cache_v3"
    pub sources: std::collections::BTreeMap<i64, SourceDirSnap>,
}
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct SourceDirSnap { pub dir: Option<String>, pub anime: std::collections::BTreeMap<String, AnimeDirSnap> }
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct AnimeDirSnap { pub dir: Option<String>, pub episodes: std::collections::BTreeSet<String> }
```

Path: `$XDG_CACHE_HOME/kurobayomi/dl_anime_index_v3.bin` (directories crate). Write with `bincode::serde::encode_to_vec` after a 1 s debounce (behaviour §8.4), via write-to-temp-then-rename.

---

## 10. GTK integration (mockup)

Existing hooks in the mockup:
- `src/screens.rs:25`: `"download_queue" => downloads(),` in the screen dispatch. Keep the match arm; `downloads()` calls the new page builder.
- `src/screens.rs:354`: `fn downloads() -> Option<Built>` builds fake groups ("Source A (3)", "Source B (2)") with fixed `0.45` progress bars. Replace the body with a call into `downloads_page::build(&manager)`.
- `src/screens.rs:134`: `download_button()` menu entries `Start downloading now`, `Cancel`, `Delete`. Wire them to `Command::StartNow`, `Command::Cancel`, `Command::DeleteEpisodes`.
- `src/entry.rs:171` and `:196`: "Download" / "Delete" menu on entry rows. Wire to `Command::Enqueue` and `Command::DeleteEpisodes`.
- `spec/app.json` id `download_queue` (line about 732) contains the Aniyomi spec text. `src/spec.rs` loads it. Use it for labels and empty text ("No downloads").

Page structure (`downloads_page.rs`):

```
Page (ui::page with back arrow, title "Download queue", count pill)
  └─ Anime / Manga tab row (ui::linked, Anime active)
      └─ ScrolledWindow
          └─ ListBox (ui::boxed_list) with set_header_func for source group headers
              └─ rows (DownloadRow): drag handle | title + episode | progress text | ProgressBar | menu button
  └─ footer: FAB-equivalent button bottom-end: "Stop" (running) or "Continue" (paused), anime tab
```

Group headers: `gtk::ListBox::set_header_func` renders "<source name> (<count>)" from the first row of each group and a drag handle. Drag: `gtk::DragSource` on each row (`gdk::ContentProvider` carrying the episode id as `i64`), `gtk::DropTarget` on each row that accepts the drop only when the dragged row's `source_id` equals the target's (the Kotlin `shouldMove` constraint, `AnimeDownloadAdapter.kt:18-21`). On drop, compute the new flat order and send `Command::Reorder`.

Row update: `watch::Receiver<i32>` and `watch::Receiver<DlState>` per row. Use `glib::spawn_future_local` with `receiver.changed().await` to update the `ProgressBar` and label. The task is dropped when the row is destroyed (drop the `JoinHandle` on `connect_destroy`).

Indeterminate: `progress == 0` → `ProgressBar::pulse()` on a 100 ms `glib::timeout_add_local`. Non-zero → `set_fraction(progress / 100)`.

Labels: `progress == 0` → "Downloading…" (MR `update_check_notification_download_in_progress`); else "Downloading (N%)" (AYMR `episode_downloading_progress`). Source-of-truth text lives in behaviour §14.

Empty state: when the anime tab has no rows, show `ui::status("folder-download-symbolic", "No downloads")`.

Tab state: the Anime/Manga selector reads from the mockup `kind_tabs` helper; the Manga tab is out of scope of this area (23-downloads-manga) and remains as is.

---

## 11. Event and update rate

- Engine publishes progress at most every 100 ms per download (10 Hz). The Kotlin 50 ms poll is too fast for both GTK and the desktop notification server.
- Notifications (`notify.rs`) update at most once per second while downloading and immediately on any state change. Behaviour §10.2 proposal.
- The page batches queue snapshot redraws: at most one rebuild per 200 ms.

---

## 12. Error handling

`src/downloads/error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum DlError {
    #[error("No video found")]                              NoVideo,
    #[error("Couldn't download due to low storage space")]  LowSpace,
    #[error("Error in ffmpeg!")]                            Ffmpeg,
    #[error("Failed to start server")]                      ServerStart,
    #[error("Launch intent not found")]                     LaunchIntentMissing,   // external only
    #[error("ffmpeg is not installed")]                     FfmpegMissing,         // new, Linux only
    #[error("Torrent server is not available")]             TorrentUnavailable,    // new, Linux only
    #[error("Invalid download location: {0}")]              InvalidLocation(String),
    #[error("{0}")]                                         Message(String),       // verbatim source text
    #[error("Cancelled")]                                   Cancelled,
    #[error(transparent)]                                   Io(#[from] std::io::Error),
    #[error(transparent)]                                   Db(#[from] rusqlite::Error),
}
```

- Keep Kotlin's user-facing texts (behaviour §14). `FfmpegMissing` and `TorrentUnavailable` are new messages and need i18n keys (area:51-i18n).
- `DlError::Cancelled` is never shown to the user.
- `DlError::Message` carries `Throwable.message` text from sources and ffmpeg wrappers, unchanged.

---

## 13. Android-API to Linux mapping (summary)

Full table in behaviour §12. Decisions that change behaviour:
- No background worker. The downloader runs while the app process runs. Closing the window with downloads active: show a "Downloads will pause" confirmation (proposal, T-24a-030). Downloads continue when the window is hidden to tray only if the app has a tray (out of scope).
- Network gating uses NetworkManager. Wired connections are allowed under the Wi-Fi-only preference (behaviour §4.3 and T-24a-031).
- Notifications use `notify-rust` with `replace_id` so each slot (progress, error) maps to one live notification, emulating IDs -203 and -204.
- `MediaScanner` is not needed. `.nomedia` is still written for interop.

---

## 14. Pitfalls for the implementer

1. Do not mutate `Download` in a `HashMap` key. Key by `episode.id`.
2. Do not run ffmpeg through a shell. Pass argv elements. The Kotlin quoting exists only because it builds a string.
3. `kill_on_drop(true)` on the ffmpeg `Child`, and call `start_kill()` explicitly on cancel, then `wait()` so the partial `.tmp` file is not held open when it is deleted.
4. Partial output on pause or cancel is deleted (Kotlin behaviour). If you change this to resume, update T-24a-016 and the edge-case doc.
5. Do not call `std::fs::rename` on the staging directory while ffmpeg still writes into it. The scheduler must await the child exit first.
6. Keep `take(3)` as the source limit. Do not use the 5 from the Kotlin comment.
7. `progress` is `i32` in the watch channel. Clamp to `0..=100`.
8. Never block the GTK thread on the DB or the filesystem. Directory walks for the cache run on the runtime.
9. The cache snapshot must be written atomically (temp file plus rename). A crash mid-write must not corrupt the snapshot; on decode failure delete it and rebuild (behaviour §8.2).
10. The queue order counter starts at 0 per process. Restore reassigns order, so do not rely on persisted `ord` values across releases.
11. Do not add a second retry layer around `with_retry`. The download task already retries the whole remux, which includes the `.tmp` cleanup.
12. Use `PathBuf::join` for every path; never concatenate separators by hand.

---

## 15. Testing hooks

- `layout::fat_safe` and `layout::episode_dir_name` are pure. Unit-test them against the table in behaviour §9.2 and T-24a-004.
- `active_set` is pure. Unit-test with synthetic queues (behaviour §18).
- `ffmpeg::build_remux_args` is pure. Golden-test against the order in §7 for: no tracks; subtitles only; audio only; both; header on/off per input; `ffmpeg_stream_args`; `ffmpeg_video_args`.
- `retry::RETRY_DELAYS_MS` and `with_retry` use `tokio::time::pause()` in tests.
- `PendingDeleter::add_episodes` batch dedupe test.
- See `p1-edge-cases-and-tests.md` for the full list.
