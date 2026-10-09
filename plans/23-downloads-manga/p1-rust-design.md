# Area 23 — Manga downloads: Rust design

Target: stable Rust 2024, gtk4-rs 0.11 (feature `v4_16`, no libadwaita), tokio, rusqlite (bundled), reqwest (through area:12), serde, tracing. Crate and runtime conventions come from area:01 (`plans/01-foundation-app-bootstrap/p1-rust-design.md`): `kuro-core` has no GTK and no OS code, `kuro-platform` is the only crate with OS-specific code, and the GUI crate is the existing `kurobayomi` package in `src/`.

Nothing in this document is implemented yet. Names are proposals unless they match an existing mockup file.

---

## 1. Crate and module layout

```
kurobayomi/
  Cargo.toml                       add crates/kuro-downloads to [workspace].members
  crates/
    kuro-downloads/                NEW, no GTK, no OS code except through kuro-platform
      Cargo.toml
      src/lib.rs                   pub use of the public API below
      src/model.rs                 DownloadState, PageState, Page, QueuedDownload, ChapterRef, MangaRef, SourceRef
      src/error.rs                 DownloadError (thiserror), mapping to message keys (section 8)
      src/layout.rs                DownloadLayout: paths and folder/file names (Prov.kt port)
      src/sanitize.rs              buildValidFilename port + reserved-name rule (addendum rule 5)
      src/index.rs                 DownloadIndex (Cache.kt port): scan, renew, persist, query
      src/store.rs                 QueueStore + PendingDeleteStore over rusqlite (Store.kt, Del.kt ports)
      src/queue.rs                 QueueState: pure in-memory state machine (no I/O, fully unit-testable)
      src/scheduler.rs             per-source selection (take 5 sources, one chapter each, skip ERROR)
      src/downloader.rs            Downloader actor: owns QueueState, runs the scheduler, emits events
      src/pipeline.rs              one chapter: precheck, page list, page fetch, integrity, finalise
      src/images.rs                image extension, tall-image split (ImageUtil port), cache copy
      src/throttle.rs              shared bytes-per-second limiter (okio Throttler port)
      src/comicinfo.rs             ComicInfo.xml writer
      src/archive.rs               CBZ writer (ZipWriter port)
      src/manager.rs               DownloadManager: public facade (MangaDownloadManager port)
      src/pending.rs               PendingDeleter logic (Del.kt port)
      src/events.rs                DownloadEvent enum (status, progress, warnings, errors)
      src/testing.rs               fake source, fake clock, temp dirs (cfg(test) or feature "testkit")
    kuro-platform/               (area:01) add: Notifier implementation details, Connectivity trait (section 5)
  src/                           GUI crate (existing mockup)
    download_queue.rs            NEW  GTK view for the manga queue (replaces screens.rs::downloads for manga)
    download_queue_model.rs      NEW  view model: groups, menu actions, sort, subscriptions
    download_notice.rs           NEW  glue: DownloadEvent -> Notifier calls (or keep in kuro-downloads)
```

Dependency direction: `kurobayomi` (GUI) -> `kuro-downloads` -> `kuro-core`, `kuro-platform`, and the domain/data crates of areas 03, 05, 07, 08, 15, 17, 27 (exposed as traits, section 3.2). `kuro-downloads` must not depend on GTK.

---

## 2. Threading model

| Thread or runtime | Owns | Communicates through |
|---|---|---|
| GTK main thread (glib main context) | all widgets, the queue view model | receives `DownloadEvent` via `async_channel::Receiver` in `glib::spawn_future_local` |
| tokio multi-thread runtime (from area:01 `kuro-core::bootstrap`) | `Downloader` actor task, page tasks, index scanner | `tokio::sync::mpsc` for commands in, `tokio::sync::broadcast` for events out |
| `spawn_blocking` pool | filesystem scans, image decoding, zip writing, rusqlite calls | results returned through `JoinHandle` |

Rules:

1. Only the `Downloader` actor mutates `QueueState`. Callers send commands. The mutex in the Kotlin code (`_queueState.update` with side effects) becomes a single owner. Store writes happen inside the actor, in the same step as the in-memory change, so a crash leaves either both or neither.
2. Status and progress are published as events, not read back: the UI keeps its own copy and applies `DownloadEvent`s. Progress events are coalesced to at most one per 50 ms per chapter (matching the Kotlin debounce, behaviour 16).
3. No blocking call on the GTK thread. `runBlocking` in the Kotlin (`startDownloadNow`, `Store.restore`) becomes `async` commands.

### 2.1 Actor skeleton

```rust
pub enum Command {
    Enqueue { manga: MangaRef, chapters: Vec<ChapterRef>, auto_start: bool, reply: oneshot::Sender<EnqueueOutcome> },
    StartDownloads,                      // Mgr.startDownloads
    Pause,                               // Dl.pause + Mgr.pauseDownloads (pause then stop)
    Clear,                               // Mgr.clearQueue
    Reorder { ordered_ids: Vec<i64> },   // Dl.updateQueue, keeps status except DOWNLOADING->QUEUE
    StartNow { chapter_id: i64 },
    AddToFront { chapters: Vec<ChapterRef> },
    Cancel { chapter_ids: Vec<i64> },    // removeFromQueue(chapters)
    CancelManga { manga_id: i64 },       // removeFromQueue(manga)
    NetworkChanged(Connectivity),
    PauseForMutation { reply: oneshot::Sender<bool /* was_running */> }, // used by delete_* in manager.rs (section 7)
    ResumeAfterMutation { was_running: bool },
    Shutdown,
}

pub struct Downloader { /* state, store, index, scheduler handles, events */ }
impl Downloader {
    pub async fn run(mut self, mut rx: mpsc::Receiver<Command>) { /* select loop, section 4 */ }
}
```

---

## 3. Key types and traits

### 3.1 Model

```rust
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DownloadState {
    NotDownloaded = 0,
    Queue = 1,
    Downloading = 2,
    Downloaded = 3,
    Error = 4,
}
impl DownloadState {
    /// Kotlin "active" test: `status.value <= DOWNLOADING.value`.
    pub fn is_active(self) -> bool { (self as u8) <= (Self::Downloading as u8) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageState { Queue, LoadPage, DownloadImage, Ready, Error }

#[derive(Clone, Debug)]
pub struct Page {
    pub index: u32,                 // 0-based, re-indexed on fetch
    pub url: String,
    pub image_url: Option<String>,
    pub file: Option<PathBuf>,      // final file once READY
    pub state: PageState,
    pub progress: i32,              // 0..=100, or -1 when the content length is unknown
}
impl Page { pub fn number(&self) -> u32 { self.index + 1 } }

pub struct ChapterRef { pub id: i64, pub manga_id: i64, pub url: String, pub name: String,
                        pub scanlator: Option<String>, pub read: bool, pub bookmark: bool,
                        pub source_order: i64, pub chapter_number: f64, pub date_upload: i64 }
pub struct MangaRef  { pub id: i64, pub url: String, pub title: String, pub source_id: i64,
                       pub description: Option<String>, pub author: Option<String>,
                       pub artist: Option<String>, pub genre: Option<Vec<String>>, pub status: i64 }

/// One queued unit. Shared with the UI as a snapshot (`Arc<QueuedDownload>`), mutated only by the actor.
pub struct QueuedDownload {
    pub source: SourceRef,
    pub manga: MangaRef,
    pub chapter: ChapterRef,
    pub status: DownloadState,
    pub pages: Option<Vec<Page>>,
}
impl QueuedDownload {
    pub fn total_progress(&self) -> i64 { self.pages.as_ref().map_or(0, |p| p.iter().map(|x| x.progress as i64).sum()) }
    pub fn downloaded_images(&self) -> usize { self.pages.as_ref().map_or(0, |p| p.iter().filter(|x| x.state == PageState::Ready).count()) }
}
```

Equality rule: queue membership is by `chapter.id` only (Kotlin `MangaDownloadItem.equals`). The Kotlin `MangaDownload` data-class equality (by manga and chapter values) is not reproduced; it caused no intended behaviour.

### 3.2 Source and domain traits (adapters to other areas)

These traits are owned by this area and implemented by the source layer (area:15, area:17). They are the only things the downloader knows about sources.

```rust
pub type SourceRef = Arc<dyn ChapterSource>;

#[async_trait::async_trait]        // or RPITIT if the toolchain allows; kept as async-trait for object safety
pub trait ChapterSource: Send + Sync {
    fn id(&self) -> i64;
    fn display_name(&self) -> String;                      // "Name (LANG)" as HttpSource.toString()
    fn name(&self) -> &str;
    fn is_unmetered(&self) -> bool;                        // UnmeteredSource marker (area:15)
    async fn page_list(&self, chapter: &ChapterRef) -> Result<Vec<Page>, SourceError>;
    async fn image_url(&self, page: &Page) -> Result<String, SourceError>;
    async fn fetch_image(&self, image_url: &str, data_saver: &dyn DataSaver) -> Result<ImageResponse, SourceError>;
    fn chapter_url(&self, chapter: &ChapterRef) -> String; // getChapterUrl(...).trim() done by caller
}

pub struct ImageResponse { pub content_type: Option<String>, pub body: reqwest::Response }

pub trait DataSaver: Send + Sync {
    fn compress(&self, image_url: &str) -> String;         // identity for NoOp
}

#[async_trait::async_trait]
pub trait SourceCatalog: Send + Sync {
    fn get(&self, source_id: i64) -> Option<SourceRef>;     // MangaSourceManager.get (HttpSource only)
    fn get_or_stub(&self, source_id: i64) -> SourceRef;     // for isChapterDownloaded(skipCache)
    fn online_and_stub_sources(&self) -> Vec<SourceRef>;
    fn gate(&self) -> &kuro_core::sources_gate::SourceGate; // area:01/17
}

#[async_trait::async_trait]
pub trait MangaStore: Send + Sync {                        // area:05 / area:07 / area:08 / area:27 facades
    async fn manga(&self, id: i64) -> Result<Option<MangaRef>, DbError>;
    async fn chapter(&self, id: i64) -> Result<Option<ChapterRef>, DbError>;
    async fn categories_of(&self, manga_id: i64) -> Result<Vec<CategoryRef>, DbError>;   // (id, name)
    async fn track_remote_urls(&self, manga_id: i64) -> Result<Vec<String>, DbError>;    // remoteUrl only
}
```

Assumption flagged: the exact signature of `ChapterSource` depends on the HTTP and source design in area:15 (`HttpSource` methods at `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:297, 335, 370, 444`). If area:15 gives different names, the adapter lives in the source crate and this trait stays as the downloader's view.

### 3.3 Layout

```rust
pub struct DownloadLayout { pub root: PathBuf }
impl DownloadLayout {
    pub fn source_dir_name(&self, source: &dyn ChapterSource) -> String;      // sanitise("Name (LANG)")
    pub fn manga_dir_name(&self, title: &str) -> String;                      // sanitise(title)
    pub fn chapter_dir_name(&self, name: &str, scanlator: Option<&str>) -> String;
    pub fn valid_chapter_names(&self, name: &str, scanlator: Option<&str>) -> [String; 2]; // [dir, dir + ".cbz"]
    pub fn manga_dir(&self, source: &dyn ChapterSource, manga_title: &str) -> PathBuf;
    pub fn find_chapter(&self, source, manga_title, name, scanlator) -> Option<PathBuf>;  // dir or .cbz
}
```

`sanitise` in `sanitize.rs`:

```rust
pub fn build_valid_filename(orig: &str) -> String {
    let trimmed = orig.trim_matches(|c| c == '.' || c == ' ');
    if trimmed.is_empty() { return "(invalid)".into(); }
    let mapped: String = trimmed.chars()
        .map(|c| if c <= '\u{1f}' || c == '\u{7f}' || "\"*/:<>?\\|".contains(c) { '_' } else { c })
        .collect();
    truncate_utf16(&mapped, 240)    // Kotlin take(240) counts UTF-16 code units
}
```

Windows rule (addendum): an additional `ntfs_safe(name)` that maps reserved device names to `name_` and removes trailing dots and spaces after truncation. Applied only when the feature `ntfs-names` is on, which is on for the Linux build too so that copied folders stay portable (decision in T-23a-003).

### 3.4 Index

```rust
pub struct DownloadIndex {
    state: RwLock<IndexState>,            // read-mostly; writes from the actor and the scanner
    last_renew: AtomicI64,                // unix millis, 0 = never
    renewing: Mutex<Option<JoinHandle<()>>>,
    persist: tokio::sync::Notify,         // debounced write, 1 s
}
pub struct IndexState { pub root: PathBuf, pub sources: HashMap<i64, SourceDir> }

impl DownloadIndex {
    pub fn is_chapter_downloaded(&self, source_id: i64, manga_title: &str, name: &str, scanlator: Option<&str>) -> bool;
    pub fn chapter_count_total(&self) -> usize;
    pub fn chapter_count_manga(&self, source_id: i64, manga_title: &str) -> usize;
    pub fn size_total(&self) -> u64;                      // recursive, spawn_blocking
    pub fn size_manga(&self, source_id: i64, manga_title: &str) -> u64;
    pub fn add_chapter(&self, source_id: i64, manga_title: &str, dir_name: &str);      // bare name
    pub fn remove_chapter(&self, source_id: i64, manga_title: &str, names: &[String]);
    pub fn remove_manga(&self, source_id: i64, manga_title: &str);
    pub fn remove_source(&self, source_id: i64);
    pub fn invalidate(&self);                             // Cache.invalidateCache
    pub async fn renew_if_stale(&self, catalog: &dyn SourceCatalog) -> bool;   // Cache.renewCache
}
```

`renew_if_stale`: return false if `now < last_renew + 3_600_000` (1 hour) or a scan is running. Otherwise: wait up to 30 s for the source gate (`tokio::time::timeout(Duration::from_secs(30), gate.wait())`). On timeout, **do not replace the index** (Kotlin replaces it with an empty map; behaviour 6.x defect). Scan folders with `spawn_blocking`; match source folders to sources by `display_name().to_lowercase()`; skip `_tmp`-suffixed chapter folders; `.cbz` files contribute their stem.

Filesystem changes outside the app: use the `notify` crate on the root (recursive, 2 s debounce) to call `invalidate` as an improvement over Kotlin, which only reacts to storage-root changes. Decision in T-23a-045.

### 3.5 Store

```rust
pub struct QueueStore { conn: Arc<Mutex<rusqlite::Connection>> }   // one writer, via spawn_blocking
impl QueueStore {
    pub fn load(&self) -> Result<Vec<QueueRow>, StoreError>;               // ORDER BY position
    pub fn replace_all(&self, rows: &[QueueRow]) -> Result<(), StoreError>;// one transaction
    pub fn upsert(&self, row: &QueueRow) -> Result<(), StoreError>;
    pub fn delete_chapters(&self, ids: &[i64]) -> Result<(), StoreError>;
    pub fn delete_manga(&self, manga_id: i64) -> Result<(), StoreError>;
    pub fn clear(&self) -> Result<(), StoreError>;
}
pub struct PendingDeleteStore { conn: Arc<Mutex<rusqlite::Connection>> }
impl PendingDeleteStore {
    pub fn add(&self, manga: &MangaRef, chapters: &[ChapterRef]) -> Result<usize, StoreError>; // INSERT OR IGNORE; returns new rows
    pub fn take_all(&self) -> Result<Vec<PendingRow>, StoreError>;                          // SELECT then DELETE in one tx
}
```

`add` returns the number of rows inserted; zero means no write (matches Del.kt's "if no chapter added, return").

`take_all` deletes inside the same transaction as the read. Proposal fix for the Kotlin crash window (behaviour 7.4): deletion of the folders happens after commit, and each chapter row is only deleted once its folder is gone; on restart rows remain and are retried.

### 3.6 Downloader state (pure, `queue.rs`)

```rust
pub struct QueueState { items: Vec<QueuedDownload> /* order = queue order */, is_paused: bool }

impl QueueState {
    pub fn add_all(&mut self, downloads: Vec<QueuedDownload>) { /* set QUEUE, append */ }
    pub fn remove_chapters(&mut self, ids: &[i64]) -> Vec<i64> { /* DOWNLOADING|QUEUE -> NOT_DOWNLOADED, return removed */ }
    pub fn remove_manga(&mut self, manga_id: i64) -> Vec<i64>;
    pub fn clear(&mut self) -> Vec<i64>;
    pub fn pause_running(&mut self) -> Vec<i64> { /* DOWNLOADING -> QUEUE, is_paused = true */ }
    pub fn stop_running(&mut self) -> Vec<i64> { /* DOWNLOADING -> ERROR */ }
    pub fn reset_for_start(&mut self) { /* non-DOWNLOADED, non-QUEUE -> QUEUE */ }
    pub fn reorder(&mut self, ordered_ids: &[i64]) -> Result<(), QueueError>; // keeps ERROR (Proposal)
    pub fn active(&self) -> Vec<&QueuedDownload>;                              // per-source first, max 5
    pub fn all_finished(&self) -> bool;                                        // none NotDownloaded/Queue/Downloading
}
```

`reorder` semantics (Proposal, behaviour 1.1): input is the full set of chapter ids in the desired order. Missing ids are an error; unknown ids are an error; ERROR is preserved (Kotlin resets it to QUEUE).

Scheduler selection (`scheduler.rs`), exact Kotlin equivalent:

```rust
pub fn pick_active(items: &[QueuedDownload], max_sources: usize /* 5 */) -> Vec<i64> {
    let mut seen: IndexMap<i64, i64> = IndexMap::new();     // source_id -> first chapter id, insertion order
    for d in items.iter().filter(|d| d.status.is_active()) {
        seen.entry(d.source.id()).or_insert(d.chapter.id);
    }
    seen.values().take(max_sources).copied().collect()
}
```

`IndexMap` (crate `indexmap`) keeps the first-occurrence order that `groupBy` gives in Kotlin.

---

## 4. The actor loop

```rust
loop {
    tokio::select! {
        Some(cmd) = rx.recv() => self.handle(cmd).await,
        Some((id, outcome)) = self.tasks.join_next_with_id() => self.on_task_done(id, outcome).await,
        ev = self.connectivity.changed() => self.on_connectivity().await,
        _ = self.progress_tick.tick(), if self.progress_dirty() => self.flush_progress(),   // 50 ms
        _ = shutdown.changed() => break,
    }
    self.reconcile().await;          // == Kotlin activeDownloadsFlow: start/stop per-chapter tasks to match pick_active()
}
```

`reconcile()`:

1. `want = pick_active(queue)`.
2. For each running task whose chapter id is not in `want`: `abort()` it, and set its status to QUEUE (see the cancel-path defect in behaviour 4). Cancelling is always done after `pause_running` in the same command, so this case is only the defensive path.
3. For each id in `want` without a task: `set_status(Downloading)` is **not** done here (Kotlin sets DOWNLOADING inside the pipeline after the precheck). Spawn `pipeline::run(chapter)` in a `JoinSet`.
4. If `all_finished()` and no tasks: `stop()`, which emits `Complete` and releases the connectivity hold.

`on_task_done(id, outcome)`:

- `Ok(Downloaded)`: remove from queue (store and events), emit `Finished`.
- `Ok(Error)` or `Err(Failed(e))`: set ERROR, emit `Error { chapter, message }`.
- `Err(Cancelled)`: nothing (status was already set by the command that cancelled).
- Panic: treated as `Error` with the panic message; the actor keeps running.

### 4.1 Pause, stop, start

| Command | Steps |
|---|---|
| `StartDownloads` | if running return; `reset_for_start()`; `is_paused = false`; take the GNOME/KDE hold (section 5); `running = true`; emit `Started` |
| `Pause` | abort all tasks; `pause_running()`; `is_paused = true`; `stop()` (below) |
| `stop(reason)` | abort tasks; `stop_running()`; if `reason`: emit `Warning(reason)` and **return with `is_paused` and running cleared and the hold released** (Kotlin keeps them, behaviour 3.6, defect); else emit `Paused` if `is_paused && !items.is_empty()` else `Complete`; clear `is_paused`; release hold |
| `Clear` | abort tasks; `clear()`; delete store rows; emit `Dismiss`; then `stop(None)` |
| `Reorder` | abort tasks (if running), `pause_running()`, `reorder`, persist `replace_all`, if it was running then `StartDownloads` |

---

## 5. Background running, connectivity and notifications

### 5.1 Keeping the process alive (replaces WorkManager, `MangaDownloadJob`)

- `kuro_platform::SchedulerBackend` (area:01) gets a `hold(reason: &str) -> HoldGuard` method. On Linux this calls `gio::Application::hold()` on the GApplication, and dropping the guard calls `release()`. The hold keeps the process alive when the window is closed while downloads run. This matches the Android behaviour of a foreground-service notification.
- Tokio tasks do not need a separate job. The actor exists for the life of the process.
- `is_running()` = `running` flag of the actor. The Android "exactly one RUNNING" check has no equivalent.

### 5.2 Connectivity (new trait in kuro-platform, Linux implementation with `gio::NetworkMonitor`)

```rust
pub struct Connectivity { pub online: bool, pub metered: bool }
pub trait ConnectivityMonitor: Send + Sync {
    fn current(&self) -> Connectivity;
    fn subscribe(&self) -> tokio::sync::watch::Receiver<Connectivity>;
}
```

Linux implementation: `gio::NetworkMonitor::default()`; `network_available()` gives online, `network_metered()` gives metered; signal `network-changed` updates the watch channel. Wi-Fi detection (`is_wifi` in Kotlin) is not available from GIO. Proposal: treat `!metered` as "not metered" and document that `download_only_over_wifi` means "not on a metered connection" on Linux. Optional later: NetworkManager D-Bus for the device type.

Gating in the actor (`on_connectivity`), replicating `checkNetworkState`:

| Online | Metered | Preference "Wi-Fi only" | Result | Warning reason key |
|---|---|---|---|---|
| no | any | any | stop, reason "No network connection available" | `download_notifier_no_network` |
| yes | yes | on | stop, reason "No Wi-Fi connection available" | `download_notifier_text_only_wifi` |
| yes | no or yes with pref off | any | continue | none |

Proposal (behaviour 5.4): on connectivity regained, resume automatically only if the queue was stopped by connectivity and the user did not press Pause. Chapters that errored only because of connectivity return to QUEUE (classify `SourceError::Network` separately).

### 5.3 Notifications (`kuro-platform::Notifier`)

Keep the four Kotlin notification classes but map them to freedesktop or GApplication notifications:

| Kotlin | Rust id | Behaviour on Linux |
|---|---|---|
| progress (-201) | `dl-progress` (replace on every update) | title = "Manga title - chapter" truncated to 30 characters, body = "Downloading (x/y)"; no progress bar (freedesktop has no progress hint; use the body text); actions: Pause, Show manga |
| paused | `dl-progress` (replaces progress) | title "Paused", body "Chapter download paused"; actions: Resume, Cancel all |
| complete | withdraw `dl-progress` | |
| warning (-202, same channel as error in Kotlin) | `dl-warning` | `Priority::Low`, `default-action` opens the Downloads page; timeout 30 s for the queue-size warning (`expire-timeout`) |
| error (-202) | `dl-error` (replace) | `Priority::Low` like Kotlin; not auto-dismissed; title "<manga>: <chapter>" |

Privacy: when `hide_notification_content` is on, the progress title is "Downloading (x/y)" only, and no manga or chapter name appears.

Notifications are sent from the actor through a channel to the GTK thread, because `gio::Notification` must be sent from the application object (`app.send_notification`). The GTK thread owns the `gio::Application`.

---

## 6. Page pipeline (`pipeline.rs`)

```rust
pub async fn run_chapter(ctx: &PipelineCtx, dl: &QueuedDownload) -> Result<ChapterOutcome, PipelineError>
pub enum ChapterOutcome { Downloaded, Errored }
```

Steps (1:1 with behaviour section 6):

1. `dir = layout.manga_dir(...)` (creates it). Failure: `InvalidLocation { path }`.
2. `free = statvfs(dir).f_bavail * f_frsize` (rustix). If `free` is known and `< 200 * 1024 * 1024`: set ERROR, emit error (`download_insufficient_space`), return `Errored`.
3. `chapter_dir_name = layout.chapter_dir_name(...)`; `tmp = dir.join(format!("{chapter_dir_name}_tmp"))`; `create_dir_all(tmp)`.
4. Page list: from the cached `dl.pages` if present, else `source.page_list(&chapter)`, error when empty (`page_list_empty_error`), re-index from 0.
5. Data saver: `dyn DataSaver` from `data_saver_for(source, prefs)` (NoOp when `data_saver_downloader` is false or the mode is NONE, or the source is excluded).
6. Delete `*.tmp` in `tmp`. Set status DOWNLOADING; emit.
7. Page fetch with `futures::stream::iter(pages).map(|p| fetch_page(p)).buffer_unordered(2)`. Each completion emits `PageDone` (coalesced progress).
8. Integrity check (behaviour 10.8, with page keys): `ready == pages.len()` and `count_page_keys(tmp) == pages.len()`.
9. ComicInfo write into `tmp`.
10. Finalise: CBZ (`archive::write_cbz(tmp, dir.join(name + ".cbz.part"))`, rename to `.cbz`, remove `tmp`) or folder rename `tmp -> dir.join(name)`. Use `rename` and check the result (Kotlin ignores it).
11. `index.add_chapter(source.id(), manga.title, bare_name)` with the bare name (no `.cbz`).
12. Status DOWNLOADED.

### 6.1 Page fetch

```rust
async fn fetch_page(&self, page: &mut Page, tmp: &Path, name_width: usize, ctx) -> Result<(), PageError> {
    if page.image_url.as_deref().map_or(true, str::is_empty) {
        page.state = PageState::LoadPage;
        match source.image_url(page).await { Ok(u) => page.image_url = Some(u), Err(_) => { page.state = PageState::Error; return Ok(()) } }
    }
    let url = match &page.image_url { Some(u) if !u.is_empty() => u.clone(), _ => return Ok(()) };   // Kotlin returns silently when null
    let stem = format!("{:0width$}", page.number(), width = name_width);  // name_width = max(3, digits(pages.len()))
    // resume: an existing finished file "stem." or "stem__001" wins
    if let Some(existing) = find_finished(tmp, &stem) { finish_page(page, existing, split_if_needed); return Ok(()) }
    if let Some(bytes) = chapter_cache.get(&url) { copy_from_cache(...) }   // see 6.2
    else { download_with_retry(...) }
}

async fn download_with_retry(...) -> Result<PathBuf, PageError> {
    let mut attempt = 0u32;
    loop {
        match download_once(...).await {
            Ok(path) => return Ok(path),
            Err(e) if attempt < 3 => { tokio::time::sleep(Duration::from_secs(2u64 << attempt)).await; attempt += 1; }
            Err(e) => return Err(e),
        }
    }
}
```

- Retry schedule: attempts 0..=3 (4 attempts); sleeps 2 s, 4 s, 8 s (`2u64 << attempt` for attempt 0, 1, 2), matching `(2L shl attempt) * 1000`.
- Retry all errors, as Kotlin does (`retryWhen` ignores the cause). Proposal: do not retry HTTP 404 and 410 (no point), keep the rest. Decision recorded as T-23a-013.
- `download_once`: `source.fetch_image(&url, &*data_saver)`; create `stem.tmp` in `tmp` (`File::create`); stream the body through the shared throttle into the file; determine extension (6.3); `rename(stem.tmp, stem.ext)`. On error: remove `stem.tmp`, drop the response, return the error.
- Progress: `page.progress` updated from bytes read over `Content-Length` (100 * read / total, -1 if unknown). Kotlin sets 0 at start and 100 on READY.

### 6.2 Cache copy

`kuro-core` provides the reader's image cache as a trait `PageImageCache { fn contains(&self, url: &str) -> bool; fn path_of(&self, url: &str) -> Option<PathBuf>; }` implemented by area:34. Copy then sniff the type. If sniffing fails, fall back to the network download (fixes the Kotlin defect in behaviour 10.4). Delete the cache file after a successful copy and rename, as Kotlin does.

### 6.3 Extension

`image::ImageReader::open(path)?.with_guessed_format()?.format()` gives the `ImageFormat`; map to `avif, gif, heif, jpg, jxl, png, webp` (Kotlin `ImageType`). If the Content-Type is `image/jpeg` use `jpg` directly (Kotlin: `ImageType.entries.find { it.mime == mime }`). `image` does not decode AVIF, HEIF, JXL out of the box. Decision: detect these by magic bytes (`ftypavif`, `ftypheic`/`mif1`, `\xFF\x0A` or the JXL container) with a small table, store the extension, and rely on the reader to decode them (area:43).

### 6.4 Tall-image split (`images.rs`)

```rust
pub fn split_tall(tmp: &Path, original: &Path, prefix: &str, optimal_height: u32) -> Result<bool, ImageError> {
    let (w, h) = image::image_dimensions(original)?;            // no full decode for the test
    if (h / w) <= 3 { return Ok(true) }                         // Kotlin integer division: h >= 4w
    if is_animated(original)? { return Ok(true) }               // GIF with >1 frame or animated WebP
    let parts = split_geometry(h, optimal_height, w);           // behaviour 10.7
    let img = image::open(original)?;                           // full decode (no BitmapRegionDecoder)
    for p in &parts {
        let sub = img.crop_imm(0, p.top, p.width, p.height);
        let name = format!("{prefix}__{:03}.jpg", p.index + 1);
        let f = File::create(tmp.join(&name))?;
        sub.to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(BufWriter::new(f), 100))?;
    }
    fs::remove_file(original)?;
    Ok(true)
}

pub fn split_geometry(height: u32, optimal: u32, width: u32) -> Vec<Part> {
    let part_count = (height - 1) / optimal + 1;
    let optimal_split = height / part_count;
    let mut out: Vec<Part> = Vec::new();
    for index in 0..part_count {
        if !out.is_empty() && height <= out.last().unwrap().bottom() { break; }
        let top = index * optimal_split;
        let mut h = optimal_split.min(height - top);
        if index == part_count - 1 { h += height - (top + h); }
        out.push(Part { index, top, height: h, width });
    }
    out
}
```

- `optimal` = `2 * max(primary monitor width, height)` in pixels (Kotlin `getDisplayMaxHeightInPx * 2`). Read from GDK (`gdk::Display::default().monitors()`), default 4096 when no display.
- Output files: `prefix__NNN.jpg` with `NNN` 1-based (Kotlin `"%03d".format(index + 1)`).
- Memory: a 50 000 px tall page decoded at 8 bit RGBA is about 800 MB. Pitfall (section 9). Proposal: decode in row bands with `png`/`jpeg-decoder` when `h * w * 4 > 256 MiB`, as a later optimisation (T-23a-018 notes it).
- The prefix bug (behaviour 10.5) is fixed here by computing the prefix from the same digit width as the file name, and by matching `starts_with(&format!("{stem}."))` or `starts_with(&format!("{stem}__"))` exactly.

### 6.5 Throttle (`throttle.rs`)

```rust
pub struct Throttle { inner: tokio::sync::Mutex<TokenBucket> }   // one per downloader, shared by all pages
struct TokenBucket { bytes_per_sec: u64, available: f64, last: Instant }
impl Throttle {
    pub async fn take(&self, n: usize) {         // n bytes; no-op when bytes_per_sec == 0
        // refill: available = min(bytes_per_sec, available + elapsed * bytes_per_sec)
        // if available < n: sleep((n - available) / bytes_per_sec); available = 0 else available -= n
    }
}
```

Speed limit read from `downloads.speed_limit_kib_s` at each chapter start, times 1024 (Kotlin reads it per download, behaviour 10.2). `0` disables the limiter. Note that okio's throttler is per-source chunk based, so exact chunk timing differs; the aggregate rate matches.

---

## 7. Public facade (`manager.rs`)

```rust
#[derive(Clone)]
pub struct DownloadManager { tx: mpsc::Sender<Command>, index: Arc<DownloadIndex>, layout: Arc<DownloadLayout>,
                             catalog: Arc<dyn SourceCatalog>, mangas: Arc<dyn MangaStore>,
                             events: broadcast::Sender<DownloadEvent>, pending: Arc<PendingDeleteStore> }

impl DownloadManager {
    // queue
    pub async fn enqueue(&self, manga: MangaRef, chapters: Vec<ChapterRef>, auto_start: bool) -> Result<EnqueueOutcome, DownloadError>;
    pub async fn start(&self);
    pub async fn pause(&self);
    pub async fn clear(&self);
    pub async fn start_now(&self, chapter_id: i64) -> Result<(), DownloadError>;
    pub async fn add_to_front(&self, chapters: Vec<(SourceRef, MangaRef, ChapterRef)>) -> Result<(), DownloadError>;
    pub async fn reorder(&self, ordered_ids: Vec<i64>) -> Result<(), DownloadError>;
    pub async fn cancel(&self, chapter_ids: Vec<i64>);
    pub async fn cancel_manga(&self, manga_id: i64);
    pub fn queued(&self, chapter_id: i64) -> Option<Arc<QueuedDownload>>;   // getQueuedDownloadOrNull
    pub fn snapshot(&self) -> Vec<Arc<QueuedDownload>>;
    pub fn events(&self) -> broadcast::Receiver<DownloadEvent>;

    // on disk
    pub fn is_chapter_downloaded(&self, source: &dyn ChapterSource, manga_title: &str, chapter: &ChapterRef) -> bool;
    pub fn chapter_count(&self) -> usize;
    pub fn chapter_count_for_manga(&self, source_id: i64, manga_title: &str) -> usize;
    pub async fn size_total(&self) -> u64;
    pub async fn size_for_manga(&self, source_id: i64, manga_title: &str) -> u64;
    pub async fn delete_chapters(&self, chapters: Vec<ChapterRef>, manga: &MangaRef, source: SourceRef) -> Result<(), DownloadError>;
    pub async fn delete_manga(&self, manga: &MangaRef, source: SourceRef, remove_queued: bool) -> Result<(), DownloadError>;
    pub async fn enqueue_pending_delete(&self, manga: &MangaRef, chapters: Vec<ChapterRef>) -> Result<(), DownloadError>;
    pub async fn flush_pending_deletes(&self) -> Result<(), DownloadError>;
    pub async fn rename_source(&self, old: &dyn ChapterSource, new: &dyn ChapterSource) -> Result<(), DownloadError>;
    pub async fn rename_chapter(&self, source: &dyn ChapterSource, manga: &MangaRef, old: &ChapterRef, new: &ChapterRef) -> Result<(), DownloadError>;
    pub async fn build_page_list(&self, source: &dyn ChapterSource, manga: &MangaRef, chapter: &ChapterRef) -> Result<Vec<LocalPage>, DownloadError>;
}

pub enum EnqueueOutcome { Queued(usize), NothingToDo, SourceNotHttp }
pub struct LocalPage { pub index: u32, pub path: PathBuf, pub is_cbz_entry: Option<String> }
```

`delete_chapters` (behaviour 7.2) and `delete_manga` (7.3) are implemented in the facade, not in the actor, but they send `Cancel` + wait for the pause/restart before touching the disk (the "pause, remove, restart" sequence of `removeFromDownloadQueue`). Implementation: `send(Command::PauseForMutation { reply })`, wait for the ack, do the disk work, `send(Command::ResumeAfterMutation { was_running })`.

### 7.1 Events

```rust
#[derive(Clone, Debug)]
pub enum DownloadEvent {
    Status { chapter_id: i64, state: DownloadState },
    Progress { chapter_id: i64, downloaded: usize, total: usize, progress_sum: i64 },
    Started, Paused, Complete, Dismiss,
    Warning { text: String, timeout_secs: Option<u32>, help_url: Option<String> },
    Error { chapter: Option<ChapterRef>, message: Option<String>, manga_id: Option<i64> },
    Finished { chapter_id: i64 },
    QueueChanged,
}
```

`Status` is emitted for every transition in section 1.1. `Progress` is coalesced (50 ms). `QueueChanged` is emitted after any change to membership or order; the view model rebuilds its groups from `snapshot()`.

---

## 8. Errors and message keys

```rust
#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("Invalid location: {0}")]           InvalidLocation(String),                 // invalid_location
    #[error("No pages found")]                  NoPages,                                 // page_list_empty_error
    #[error("Couldn't download due to low storage space")] LowSpace { free: u64, need: u64 },  // download_insufficient_space
    #[error("Page {0} not found while splitting")] SplitPageNotFound(u32),                // logged only
    #[error(transparent)]                      Source(#[from] SourceError),             // raw text shown
    #[error(transparent)]                      Io(#[from] std::io::Error),
    #[error(transparent)]                      Db(#[from] StoreError),
    #[error("Cancelled")]                      Cancelled,                               // never shown
}
```

- User-visible text: the English strings from `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/.../base/strings.xml`, loaded by area:51 (`kuro-i18n`). The Rust code uses the keys, not the English literals.
- Raw error text from sources is shown verbatim, as Kotlin does (behaviour 15), but logged with the full chain.
- `SourceError::Network` marks retryable connectivity errors for section 5.2.

---

## 9. Pitfalls (must be handled in the port)

1. **Index replaced by an empty map on timeout** (Kotlin Cache.kt `withTimeoutOrNull`): do not replace; keep the old index, log, and retry later.
2. **Restore drops rows whose source is not ready** (Store.kt `continue`): keep rows until the gate opens.
3. **Busy-wait in the job** (MangaDownloadJob.kt loop): use `watch` channels; never poll.
4. **Status not reset when a job is cancelled** (behaviour 4): every abort path sets the chapter status explicitly.
5. **`updateQueue` resets ERROR to QUEUE**: keep ERROR on reorder.
6. **`stop(reason)` skips clearing the paused flag and stopping the job**: always clear.
7. **Prefix collision for pages 1000+** (behaviour 10.5): use the same width everywhere.
8. **`.cbz` in the index for renamed chapters**: store bare names.
9. **Copy from cache leaves `.tmp` with unknown type**: fall back to network.
10. **Large tall images**: a full decode of a 50 000 px tall RGBA page needs about 800 MB. Decode in bands or cap the decode size and skip the split above a limit (Proposal).
11. **Folder rename result ignored**: check and fail the chapter when the rename fails.
12. **`.cbz_tmp` leftovers**: write `.cbz.part`, delete on failure, clean stray files at startup (T-23a-044).
13. **Two chapters with the same folder name**: the second is reported as downloaded (Kotlin behaviour). Proposal: detect the collision at enqueue time and show a warning.
14. **Path separators and case**: use `Path`/`PathBuf` only. Index matching of source folders is case-insensitive on purpose (Kotlin `lowercase()`), which matters on case-insensitive file systems.
15. **Notification order**: `gio` replaces notifications by id; send the paused notification with the same id as progress so the old progress text is replaced.
16. **`hashCode` truncation** in Kotlin `MangaDownloadItem.hashCode` (`chapter.id.toInt()`): do not copy.

---

## 10. Android API → Linux mapping (for this area)

| Android / Kotlin API used here | Linux / Rust replacement | Where |
|---|---|---|
| WorkManager `CoroutineWorker`, `OneTimeWorkRequest`, `enqueueUniqueWork(REPLACE)` | tokio actor + `gio::Application::hold()` guard (`SchedulerBackend::hold`) | section 5.1 |
| `ForegroundInfo` / `setForegroundSafely` / `FOREGROUND_SERVICE_TYPE_DATA_SYNC` | not needed; the hold keeps the process alive; a tray-free window status is shown in the Downloads page | 5.1 |
| `ActiveNetworkState`, `networkStateFlow` (ConnectivityManager) | `gio::NetworkMonitor` (`network-changed`, `network_available`, `network_metered`) | 5.2 |
| `NotificationCompat`, channels, groups, `setOnlyAlertOnce`, `setProgress`, `setTimeoutAfter`, actions | `gio::Notification` (`set_priority`, `add_button`, `set_default_action`) or freedesktop via `notify-rust` | 5.3 |
| `SharedPreferences` (`active_downloads`, `chapters_to_delete`) | SQLite tables (section 3.1, data file) | 3.5 |
| `ProtoBuf` disk cache (`dl_index_cache_v3`) | `bincode` snapshot (`download-index-v1.bin`) | 3.4 |
| `UniFile` / Storage Access Framework / `DocumentFile` | `std::fs` with `Path`; `Paths` trait for the root | 3.3 |
| `StatFs` (`getAvailableStorageSpace`) | `rustix::fs::statvfs` | 6 step 2 |
| `MediaScannerConnection.scanFile` (`.nomedia`) | not used; `.nomedia` written once at the root if chosen | data file 4.2 |
| `ZipWriter` (libarchive, STORE, UTF-8) | `zip` crate, `CompressionMethod::Stored`, `SimpleFileOptions` | archive.rs |
| `BitmapFactory`, `ImageDecoder`, `BitmapRegionDecoder`, `Bitmap.compress(JPEG, 100)` | `image` crate (`ImageReader`, `crop_imm`, `JpegEncoder::new_with_quality(_, 100)`) | 6.3, 6.4 |
| `Resources.getSystem().displayMetrics` | `gdk::Display` monitors, geometry | 6.4 |
| `xmlutil` `XML.encodeToString(ComicInfo)` | `quick-xml` `Writer` with explicit namespaces | comicinfo.rs |
| `kotlinx.serialization` JSON | `serde_json` (only for the Kotlin backup import, none in this area) | 3.5 |
| `okio.Throttler` | `throttle.rs` token bucket | 6.5 |
| `ChapterCache` (`DiskLruCache`) | area:34 trait `PageImageCache` | 6.2 |
| `Mutex` (kotlinx) | `tokio::sync::Mutex` in async code, `parking_lot::Mutex` in sync | — |
| `MutableStateFlow` / `StateFlow` | `tokio::sync::watch`; UI copy updated from events | 2 |
| `Channel.UNLIMITED` + `shareIn(replay 1)` (cache changes) | `tokio::sync::broadcast` with capacity 64 | 3.4 |
| `storageManager.changes` | `notify` crate watcher on the root (decision T-23a-045) | 3.4 |
| `runBlocking` on UI (`startDownloadNow`, restore) | async commands; never blocking on GTK | 2 |
| `logcat` | `tracing` with target `kuro_downloads` | throughout |
| Voyager `ScreenModel`, `screenModelScope` | GTK view model with `glib::spawn_future_local` | ui-parity, section 'View model' |
| Compose `AndroidView(RecyclerView)` | GTK `ListView` with `GtkDragSource`/`GtkDropTarget` | ui-parity |

---

## 11. Plugging into the existing mockup

Current state (`src/screens.rs:354-397`, `fn downloads()`): builds two hard-coded groups ("Source A (3)", "Source B (2)"), fixed progress 0.45 and labels "12/30" or "45%", a FAB toggling "Pause" and "Resume" labels, and a sort menu and row menus whose callbacks are `|_| {}`. The `kind_tabs` helper (screens.rs:70) provides the Anime and Manga switch; the Manga side is what this area replaces.

Plan:

1. Keep `kind_tabs` and the sort-menu entries (`screens.rs` lines around 376-377) but change the callbacks: `menu_button(icon, entries, on)` already passes the label (`on: impl Fn(&str)`, `ui.rs:202`). Dispatch on the label to `DownloadManager` calls (the action names `move_to_top`, `move_to_top_series`, `move_to_bottom`, `move_to_bottom_series`, `cancel_download` and `cancel_series` keyed by a stable id; see ui-parity section 1.5).
2. Replace the body of the manga branch with `download_queue::view(model)`, where `model: Rc<DownloadQueueModel>` lives on the GTK thread and subscribes to `DownloadManager::events()`.
3. The spec file `spec/app.json` screen `download_queue` (lines 732-752) already describes the layout; keep using `screen("download_queue")` for the title, count pill, and empty text "No downloads" (`spec.rs` `Screen` fields). The row, header and FAB geometry come from the spec notes.
4. FAB state comes from `DownloadManager::running()` (the actor's `running` flag), not from the label text; the current code reads the label back (`lbl.label() == b`), which is a mockup shortcut.
5. The Updates page (`spec/app.json` line 539) shows a per-row download indicator from `DownloadManager::queued(chapter_id)`; that wiring is area:42/37, not this area.

No changes to `ui.rs` are needed for the queue; `menu_button`, `list_row`, `heading`, `boxed_list`, `fab` (screens.rs) are reused.

---

## 12. Testing hooks

- `QueueState`, `pick_active`, `reorder`, `split_geometry`, `build_valid_filename`, `DownloadLayout` names, `split_tall` decisions, `Throttle` arithmetic, and the retry schedule are pure functions. Unit tests run without a runtime (or with `tokio::time::pause()` for the retry schedule).
- `DownloadIndex` is tested against a temp directory tree built by `testing.rs`.
- The pipeline is tested with a `FakeSource` implementing `ChapterSource` with scripted responses (success, 500, 404, empty page list, unknown content type, huge Content-Length). See the test list in `p1-edge-cases-and-tests.md`.

---

## 13. Versions and features

- Crates (new): `indexmap`, `async-trait`, `zip`, `quick-xml`, `notify` (optional, T-23a-045), `bincode`, `rustix` (already in area:01), `image` (area:13 decides the feature set; needs `jpeg`, `png`, `gif`, `webp`, `avif` decode if available, and encode `jpeg`).
- Feature flags on `kuro-downloads`: `ntfs-names` (default on), `external-tests` (off).
- No new system libraries. `image` AVIF decoding may need `dav1d` at build time; if it does, AVIF stays a "recognised but not split" type.
