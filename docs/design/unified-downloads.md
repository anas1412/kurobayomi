# Unified downloads (step 5: one download system)

Source: `docs/design/unify-map.md` (step 5 row, sections 1b, 1c, 2, 3, and the Downloads drift table), `docs/design/unified-model.md`
(the `Entry`/`Item` model step 4 finished), `docs/design/unified-db.md` (the `download_*` tables), and a read of
`crates/kuro-downloads/**` (16,300 lines with tests), `src/downloads/**`, `src/updates/download_host.rs`,
`src/shell/host.rs`, `src/library_update/renames.rs`, `src/reader/{delete_disk,delete_queue,download_paths,launch}.rs`,
`src/entry_core/{backend,downloaded,anime/download_backend}.rs`, and every other caller of `kuro_downloads` on 2026-10-13
(main at ff4f785). Read only. Nothing was built or run.

## 0. What the read changed about the map's picture

Four facts the map did not have. They shape the design.

1. **Nothing feeds the network gate, for either kind.** `src/downloads/mod.rs:158` creates the manga `Connectivity` watch as
   `online: true, metered: false` and keeps the sender; no code ever sends on it. `AnimeQueue::network_changed` has no caller.
   So "network off stops both" is not a drift between the kinds, it is not wired at all. The same holds for `ProcessHold`:
   the app passes `NoHold` (`mod.rs:168`).
2. **The manga startup sweep deletes every staging folder.** `DownloadManager::spawn` builds its keep list from
   `params.restored`, and the app passes `restored: Vec::new()` (`mod.rs:169`); the restore happens later in
   `spawn_restore`. `sweep_stray_chapters(root, &[])` therefore removes every `<chapter>_tmp` folder at every start, so a
   resumed manga chapter always refetches every page. Anime restores before `spawn`, so its sweep keeps the right folders.
3. **A cancel retries every failed manga chapter.** `Downloader::remove` pauses the run, removes, then `resume_run` calls
   `start_run(true)` (explicit), and an explicit start runs `reset_for_start`, which puts every ERROR chapter back in QUEUE.
   Anime `remove_episodes` does not.
4. **Pending deletes are unwired in three places.** `DownloadManager::{defer_delete, flush_pending_deletes}` and the anime
   `EnqueueDelete`/`FlushPendingDeletes` have no caller. The reader has its own third path: a JSON file
   (`reader_delete_queue.json`) and `DiskDeleter`, manga only, which guesses the source folder by walking every folder
   under the root. Anime has no delete-after-watch at all (step 9 adds the hook; this step gives it a place to call).

Also already shared today (so the design reuses it instead of inventing it): `DownloadState` and its transition table,
`scheduler::pick_active`, `retry::with_retry`, `delete::select_for_delete`, `queue_model::{QueueItem, QueueModel}`,
`SeriesLocks` (keyed by `(Kind, id)`), `pipeline::MIN_FREE_BYTES`, `FreeSpaceFn`, the `NotificationSink` trait and the
`DownloadEvent` type. The unification is mostly the actor, the stores, the index, the layout and the notifier.

---

## 1. Inventory

### 1.1 Duplicated pairs (29)

Lines are `wc -l` of the whole file including tests. "Both" means one copy serves both kinds already.

| # | Pair (what it is) | Manga | Anime | What differs |
|---|---|---|---|---|
| 1 | Queue actor loop | `downloader.rs` 984 (actor about 780) | `anime/manager.rs` 1641 (actor about 1,060) | Manga: tokio `JoinSet`, per-task generation, 50 ms progress flush, `spawn_blocking` for the save. Anime: bare `tokio::spawn` plus `CancellationToken`, sync save on the actor, no progress events. Same idea, different bugs (section 1.2). |
| 2 | Handle and `Command` enum | `manager.rs` 1517 (`DownloadManager`, `Command`, bounded `mpsc(64)`, `try_send` drops) | `AnimeQueue` + `Command` in `anime/manager.rs` (unbounded) | Different command names and replies; `EnqueueOutcome` is an enum for manga, a struct for anime. |
| 3 | Queue state machine | `queue.rs` 437 (`QueueState`, items replaced to change status) | inline `Vec<Arc<AnimeDownload>>` in the actor, status in a `watch` cell | Anime has no pure state type, so no unit tests of reorder/requeue rules. |
| 4 | Queued item and refs | `model.rs` 216 (`QueuedDownload`, `ChapterRef`, `MangaRef`, `Page`, `SourceRef`) | `anime/model.rs` 215 (`AnimeDownload`, `EpisodeRef`, `AnimeRef`, `AnimeSource`, `Video`) | Same fields under two names (`read`/`seen`, `chapter_number`/`episode_number`); anime adds `fillermark`, `alt`, `video`, live `ProgressCell`. |
| 5 | Queue store | `store.rs` 220 (`QueueStore`, unreadable guard, writes `status`) | `anime/store.rs` 176 (`AnimeQueueStore`, no guard, never writes `status`) | Both write `download_queue` with a kind literal; anime fills `source` by sub-select. |
| 6 | Pending-delete store | `pending.rs` 204 (`MangaPendingStore`, filters at enqueue, takes every row) | `anime/pending.rs` 218 (`AnimePendingStore`, takes only installed sources, moves corrupt rows aside) | Anime keeps `done`/`bookmark`; manga writes 0. |
| 7 | Layout and names | `layout.rs` 255 + `sanitize.rs` 56 (root required) | `anime/layout.rs` 415 + `anime/naming.rs` 186 (root `Option`) | Same `<root>/<source>/<title>/<unit>` tree and the same sanitiser. Unit folder rule differs (`.cbz` or folder vs one folder), anime adds collision naming and rename helpers. |
| 8 | Download index | `index.rs` 722 (`DownloadIndex`, owned by Shell) | `anime/cache.rs` 689 (`AnimeIndex`, owned by a `OnceLock` in `download_host.rs`) | Different tree types, snapshot files (`download-index-v1.bin` / `dl_anime_index_v3.bin`) and scan rules (`.cbz` stems vs `.mkv`/`.mp4` stems). Anime has `changes()` watch and a cheap `Arc<Tree>` read; manga has the "Checking downloads" indicator. |
| 9 | Root watcher | `watch.rs` 94 (started by `DownloadManager::spawn`) | none | Anime index never invalidates on external changes. |
| 10 | Delete rules | `manager.rs:75` `DeleteRules` + `delete.rs` 101 (shared filter) | `anime/delete_rules.rs` 73 | Field names only (`manga_categories` / `item_categories`); the filter itself is shared. |
| 11 | Disk delete, rename, prune | `manager.rs` about 320-620 (`delete_chapters`, `delete_manga_locked`, `rename_*`, `remove_path` NotFound-tolerant) | `anime/manager.rs` about 955-1030 + `anime/layout.rs` rename fns | Anime `remove_path` is not NotFound-tolerant; anime `DeleteAnime` pauses the whole run, manga pauses only a run that holds a chapter of that title. |
| 12 | Pending flush | `manager.rs` `flush_pending_deletes` + `pending::flush_plan` | `anime/manager.rs` `flush_pending` | Manga reads no live bookmark flag (rows carry 0); anime takes a live map. Both uncalled. |
| 13 | Enqueue filter and bulk warning | inline in `downloader.rs` (`enqueue_placed`, `warn_if_bulk`, thresholds 30/15) | `anime/enqueue.rs` 145 (thresholds 20/10); third copy of 30/15 in `kuro-domain/src/download/policy.rs` | Decided 30/15. |
| 14 | Connectivity gate | `downloader.rs` (`Connectivity`, `gate`, `on_connectivity`) | `anime/network.rs` 260 (`NetworkGate` 2 s debounce, `NetworkProbe`, `SysfsNetwork`) | Two policies, neither fed (section 0). |
| 15 | Process hold | `downloader.rs` `ProcessHold`, `NoHold` | none | App passes `NoHold`. |
| 16 | Run state | `running` bool + `paused` flag in `QueueState` | `anime/lifecycle.rs` 54 (`RunnerState`), unused outside tests | |
| 17 | Events | `events.rs` 31 (`DownloadEvent`, chapter-flavoured: `chapter_id`, `manga_title`) | same enum, only `Warning` and `Error` sent | Anime "done" is found by the app polling every second. |
| 18 | Notifier | `notify.rs` 356 (`spawn_notifier`, ids `dl-*`) | `anime/notify.rs` 371 (`AnimeNotifier`, ids `dl-anime-*`, error digest) | Two texts, two id sets, error digest only anime. |
| 19 | Engine adapters | `engine.rs` 381: `MangaEngine` | `AnimeEngine`, plus `manga_model` / `anime_model` | The queue page and live adapter branch on kind through these. |
| 20 | Series locks | `manager.rs:92` `SeriesLocks` (used by manga deletes) | borrowed in `DeleteEpisodes` only | `DeleteAnime`, flush and renames take no lock. |
| 21 | Download prefs | `kuro-downloads/src/download_prefs.rs` 86 + `settings.rs` 47 | `kuro-domain/src/download/prefs.rs` 68 | Two `DownloadPrefs` for the same four keys; `misc/runner.rs:23` uses one, `settings_ui` and the anime queue the other. |
| 22 | Startup and restore (app) | `src/downloads/mod.rs` 197 (`start_manga`, `spawn_restore`, `Waiting`, `cancel_waiting`) | `src/downloads/anime_queue.rs` 222 (`restore`, `restore_entry`, `notices` poll loop) | Manga restores after sources load, keeps unknown-source rows as Waiting, repeats on source change. Anime restores once before sources load, drops rows without a name, never shows Waiting. |
| 23 | Source ports (app) | `src/downloads/manga_io.rs` 151 (`LiveChapterSource`) | `src/downloads/anime_io.rs` 181 (`LiveHosters`, `HttpVideoFetch`) | Kind-specific, stays two (section 2). |
| 24 | Live adapter and queue page (app) | `live.rs` 148, `queue_page.rs` 306: `Entry::{Manga, Anime}`, `items_of`, `running`, `for_each_engine_of`, `move_item`, `status_line` all branch on kind | | A 4 Hz poll of both queues. Waiting rows are manga-only. |
| 25 | Entry-page download backend (app) | `entry_core/backend.rs` about lines 40-60, 176-340, `entry_core/downloaded.rs` 60 | `entry_core/anime/download_backend.rs` 220 (own `OnceLock` queue, disk-stat `downloaded`) | Manga reads the index, anime stats the disk; explicit delete forces `remove_bookmarked` on manga only. |
| 26 | Index owners (app) | `shell/host.rs:83` `Shell::downloads()` | `updates/download_host.rs:137` `anime_index()` | Two owners, two snapshot writers, two rescan hooks. |
| 27 | Downloaded facts (app) | `library/download_facts.rs`, `library/mount.rs:295`, `browse/migrate_live.rs`, `download_host.rs` `IndexBackend::{on_disk, entry_usage}` | the same four places answer `false` / `0` for anime | |
| 28 | Folder renames (app) | `library_update/renames.rs` `rename_chapter` (updates the index) | `rename_episode` (never touches the index) | |
| 29 | Delete after read (app) | `reader/delete_disk.rs` 226 + `delete_queue.rs` 189 (`QueueFile`) + `download_paths.rs` 62 | none | Third delete path, manga only. |

Plus one cross-cutting copy that is not a pair: `src/ext_host.rs:101-102` builds orphan source folder names with
`DownloadLayout::source_dir_name(&ext.name)` for manga (no `(LANG)`) and `anime::naming::source_dir_name(name, lang)` for anime.

### 1.2 Every real behavioural difference

Class: **bug** (wrong in one or both kinds), **drift** (the kinds disagree, one rule must win), **need** (the kinds really differ, stays two).
"Rule" is what the unified code does.

| # | Difference | Manga | Anime | Class | Rule |
|---|---|---|---|---|---|
| B1 | Active sources | `MAX_ACTIVE_SOURCES = 5` hard-coded (`scheduler.rs:8`), the setting is ignored | `max_active` closure reads the "Download slots" pref (clamped 1 to 5) | drift (decided: the user's setting) | One closure for both; default value in decision D1. |
| B2 | Bulk warning thresholds | 30 total / 15 per source | 20 / 10 | drift (decided 30/15) | One `BULK_*` pair in `unit.rs`; the `kuro-domain` copy is deleted. |
| B3 | When the warning shows | `auto_start && (was_empty or front)` | `auto_start && was_empty` | drift | Manga rule (a front enqueue starts a run too). |
| B4 | Unreadable-queue guard | `QueueStore.unreadable`: a failed `load` blocks every later write | none: a failed read followed by a save wipes the queue | **bug** (anime, data loss) | Guard in the one store, per kind. |
| B5 | Restore timing | after sources load, again on every source change | once, before sources load | drift | Restore at spawn; `SourcesChanged` re-evaluates. |
| B6 | Row whose source is not installed | kept, shown as "Waiting for source" | dropped when no source name is known, else queued as a stub | drift | Kept as an ordinary queued item with `waiting = true` (both kinds). |
| B7 | Persisted `status` | written | never written (default 1) | drift | Written for both; restore always forces QUEUE. |
| B8 | Blocking work on the actor | save in `spawn_blocking` | `persist()`, `flush_pending`, `delete_anime_folder` run sync on the actor | **bug** (anime) | Every store call and disk operation runs in `spawn_blocking`. |
| B9 | Series locks | `delete_chapters`, `delete_manga` | `DeleteEpisodes` only | **bug** (anime) | One lock for delete, flush, rename, prune, for both kinds. |
| B10 | NotFound-tolerant remove | yes (`manager.rs:534`) | no (`anime/manager.rs:1013`) | **bug** (anime) | One `Layout::remove_path`. |
| B11 | "Is it downloaded" | index | disk stat (`find_episode_dir`) also on title pages | drift | Index everywhere; the runner re-checks the disk before it starts a unit (manga already does). |
| B12 | Finalise collision | replace the old copy | refuse, then name the folder `Name (id)` | drift (decided: replace) | Replace for both; `unique_episode_dir_name` and its resume rule go. |
| B13 | Events | Status, Progress, Started, Paused, Complete, Finished, QueueChanged, Warning, Error | Warning, Error only | drift | The actor emits the full set for both kinds; the 1 s poll loops go. |
| B14 | Network | gate written, never fed | gate written, never fed | **bug** (both) | One `Gate`, fed by `gio::NetworkMonitor` in the app. |
| B15 | Explicit start | retries every ERROR unit | requeues every unit that is not QUEUE | drift | Manga rule: explicit start retries ERROR; an automatic resume retries only units the network stopped. |
| B16 | Resume after a mutation (remove, reorder, delete) | calls the explicit start, so it retries every ERROR unit | does not | **bug** (manga) | Resume after a mutation retries nothing. |
| B17 | `InvalidLocation` stops the whole run | yes | no | drift | Stops the lane, both kinds. |
| B18 | Process hold | trait, `NoHold` in the app | none | **bug** (unwired) | Decision D4. |
| B19 | Root watcher | yes (manga root) | none | drift | One watcher over both roots. |
| B20 | Startup sweep keeps restored staging | broken (section 0 fact 2) | works | **bug** (manga) | Sweep runs after restore with the restored units as the keep list. |
| B21 | Max-downloads label | the pref governs only anime; label says "anime" | | drift | Label "Maximum downloads"; one pref. |
| B22 | `Downloading` set | inside the pipeline, after the page list is fetched | at spawn | drift | At spawn (the item is running from that moment). |
| B23 | Reorder that changes nothing | pauses and restarts the run | no-op (T-24a-052) | drift | No-op. |
| B24 | Enqueue outcome | enum `Queued(n)`/`NothingToDo`/`SourceNotHttp` | struct `{added, started, warning, continue_toast}` | drift | The struct; `SourceNotHttp` becomes `added = []` plus `refused: Option<Refusal>`. |
| B25 | `start_now` of a unit already on disk | not checked | `AlreadyDownloaded` | drift | Refused for both. |
| B26 | Error toast digest | none | five-second digest ("3 downloads failed") | drift | Digest for both, per kind. |
| B27 | Pending flush scope | takes every row | takes only rows whose source is installed, others wait (E-P01) | drift | Rows whose source cannot be named stay; the rest are taken. |
| B28 | Corrupt pending rows | none handled | moved to `download_pending_corrupt` | drift | Moved aside for both. |
| B29 | Failed flush group | re-added through the filter with `remove_bookmarked: true` | restored as it was | drift | Restored as it was. |
| B30 | Pending rows carry `done`/`bookmark` | writes 0 | writes the live values | drift | Live values; the flush also reads the live flags from the database. |
| B31 | Explicit delete and bookmarks | forces `remove_bookmarked` | passes the default rules, so a bookmarked episode survives an explicit delete | **bug** (anime) | `DeleteRules::EXPLICIT` for every explicit delete. |
| B32 | Delete pauses | only a run that holds a unit of that entry | pauses the whole run | drift | Per-entry pause (`hold_entry`). |
| B33 | Max-sources scheduling of Waiting items | n/a | n/a | new | A waiting item is skipped like an ERROR item (`pick_active` skips it). |
| B34 | Fillermarked units | none exist | dropped at enqueue unless the cut pref `download_fillermarked` is on (hard-coded off), also for an explicit pick | drift | Decision D6. |
| B35 | Orphan source folder names | no `(LANG)` | `(LANG)` | drift | `Layout::source_dir_name(&SourceKey)` for both. |
| B36 | Anime-only `alt` and `video` enqueue parameters | | the app always passes `alt: false`, `video: None` | dead | Dropped. |
| B37 | Page list kept across a pause | `QueuedDownload.pages` | n/a | need | Kept inside `ChapterPipeline` (a small map), not in the shared item. |
| B38 | Progress shape | pages (`x of N`) and a percent | percent | need | `UnitProgress { percent, pages: Option<(done,total)> }`. |
| B39 | Unit on disk | folder or `<name>.cbz` | one folder holding one `.mkv`/`.mp4` | need | Per-kind rule in `naming.rs` (unit names, scan extensions). |
| B40 | Staging | `<name>_tmp` folder and `<name>.cbz.part` | `<name>_tmp` folder holding `<name>.tmp` | need | `UnitRunner::staging_names`; one sweep walks both. |
| B41 | The run itself | page list, page fetch, tall-image split, ComicInfo, `.cbz` | resolve hoster, HLS/ffmpeg remux, torrent, local server | need | The two `UnitRunner`s. |
| B42 | Page cache, speed limiter | `PageImageCache` (app passes `None`), `SpeedLimiter` (always unlimited) | none | need / dead | Stay inside `ChapterPipeline`; dead-code candidates for step 12. |

Manga-side numbers worth keeping in mind: `MIN_FREE_BYTES` (200 MB) is already shared by both runners; `RETRY_DELAYS` (2, 4, 8 s) is
shared. Neither changes.

---

## 2. Target design

### 2.1 Where it lives and how the old and new coexist

`kuro-downloads` keeps its role (no GTK, no OS code) and gains dependencies on `kuro-data` (for `Entry`, `Item`, the two repos) and
`kuro-platform` (already: `Kind`). `kuro-data` does not depend on `kuro-downloads`, so there is no cycle.

New code goes in `crates/kuro-downloads/src/next/` during phases A to C, beside the old modules. Phase D deletes the old
modules and moves `next/*` up one level (`git mv`, mechanical). This is the same expand-then-contract idea as step 4, with a
directory instead of wrappers, because the queue types change shape and a wrapper over a different item type would carry logic.

Final layout after phase D (`kuro-downloads/src/`):

```
lib.rs  unit.rs  state.rs  runner.rs  actor.rs  queue.rs  store.rs  pending.rs  restore.rs  service.rs
layout.rs  naming.rs  index.rs  watch.rs  delete.rs  events.rs  notify.rs  gate.rs  locks.rs  locate.rs
model.rs (DownloadState only)  queue_model.rs  retry.rs  scheduler.rs  error.rs  redact.rs  testing.rs
manga/   pipeline.rs  images.rs  comicinfo.rs  source.rs  runner.rs  pages.rs (build_page_list)  local.rs
anime/   downloader.rs (EpisodeEngine)  ffmpeg.rs  resolve.rs  local_server.rs  torrent.rs  offline.rs  progress.rs  runner.rs
```

What is generic over `Kind` and what is a per-kind strategy:

| Generic (one copy, takes or stores a `Kind`) | Per-kind strategy (small, behind a trait or a table) |
|---|---|
| `QueuedUnit`, `Lane`, `Queue` actor, `Snapshot`, `Command` set | `UnitRunner` impl: `manga::ChapterPipeline`, `anime::EpisodeEngine` |
| `QueueStore`, `PendingStore`, `restore` | `naming.rs`: unit folder name(s), scan extensions, staging names (a `match kind` over pure functions) |
| `Layout` (tree walk, lookup, rename, prune, sweep, NotFound-tolerant remove) | the source port: `ChapterSource` (pages) and `HosterResolver` + `VideoFetch` (video) |
| `DownloadIndex` (one tree type, one snapshot file, one watcher) | `locate` consumers: reader opens pages (`manga::pages`), player opens a file (`anime::offline`) |
| `DeleteRules`, `Downloads` service, `SeriesLocks`, `Gate`, `DownloadNotifier`, `DownloadEvent` | progress display text (`pages_total` present or not) |

### 2.2 Shared value types (`next/unit.rs`)

They are projections of `kuro_data::entry::Entry` and `kuro_data::item::Item`, not the full rows. Reason: a pending-delete row
and a Waiting row know a title, a url, an id and a name, not a whole `Entry`, and a cloned `Entry` per queued unit is heavy
(the `Entry` struct has about 30 fields). `From<&Entry>` and `From<&Item>` are the only place the rows are read.

```rust
use kuro_data::{entry::Entry, item::Item};
use kuro_platform::kind::Kind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryRef { pub id: i64, pub kind: Kind, pub source: i64, pub url: String, pub title: String }
impl From<&Entry> for EntryRef { fn from(e: &Entry) -> Self; }

#[derive(Clone, Debug, PartialEq)]
pub struct UnitRef {
    pub id: i64, pub entry_id: i64, pub url: String, pub name: String, pub scanlator: Option<String>,
    pub done: bool, pub bookmark: bool, pub fillermark: bool,
    pub source_order: i64, pub date_upload: i64, pub number: f64,
}
impl From<&Item> for UnitRef { fn from(i: &Item) -> Self; }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability { Live, Local, Stub }          // Local: refused at enqueue; Stub: not installed, the unit waits

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceKey { pub id: i64, pub name: String, pub lang: String, pub unmetered: bool, pub availability: Availability }
impl SourceKey {
    pub fn display_name(&self) -> String;            // "Name (LANG)", or "Name" when lang is empty
}

/// Live numbers a runner writes and the actor reads. Lock-free so a runner never waits for the actor.
#[derive(Default)]
pub struct UnitProgress { /* percent: AtomicU8, done: AtomicU32, total: AtomicU32 (0 = unknown), version: AtomicU64 */ }
impl UnitProgress {
    pub fn set_percent(&self, percent: u8);
    pub fn set_pages(&self, done: u32, total: u32);
    pub fn percent(&self) -> u8;
    pub fn pages(&self) -> Option<(u32, u32)>;
    pub fn version(&self) -> u64;                     // bumps on every set, the actor flushes when it moved
}

/// One unit in the queue. Immutable: a state change swaps the `Arc`. `progress` is the only live part.
pub struct QueuedUnit {
    pub kind: Kind, pub entry: EntryRef, pub unit: UnitRef, pub source: SourceKey,
    pub state: DownloadState,                         // Queue | Downloading | Error (Downloaded never stays queued)
    pub waiting: bool,                                // true: the runner cannot reach the source; skipped by the scheduler
    pub progress: Arc<UnitProgress>,
}

pub const BULK_TOTAL_THRESHOLD: usize = 30;
pub const BULK_PER_SOURCE_THRESHOLD: usize = 15;
```

`DownloadState` (`model.rs`) is kept as is: five values, the same `can_become` table. "Waiting" is a flag, not a sixth state,
because the persisted `status` column, the transition table, the download button and every `match` on `DownloadState`
would otherwise change.

### 2.3 The runner trait (`next/runner.rs`)

This is the "page fetch to .cbz vs ffmpeg to .mkv" seam the map keeps as two.

```rust
#[async_trait]
pub trait UnitRunner: Send + Sync {
    fn kind(&self) -> Kind;
    /// Whether the source can be reached now (installed). `false` keeps the unit queued as Waiting.
    fn can_run(&self, source: &SourceKey) -> bool;
    /// Staging folder names of a unit under its entry folder, kept by the startup sweep and the cancel cleanup.
    fn staging_names(&self, unit: &UnitRef) -> Vec<String>;
    /// Runs one unit to the end. `Ok` names the finished folder or file stem, which the actor records in the index.
    /// `Err(Cancelled)` means a pause or cancel, anything else puts the unit in ERROR.
    async fn run(&self, job: UnitJob, ctx: RunCtx) -> Result<UnitDone, DownloadError>;
}

pub struct UnitJob { pub entry: EntryRef, pub unit: UnitRef, pub source: SourceKey }
pub struct UnitDone { pub dir_name: String }
pub struct RunCtx {
    pub cancel: tokio_util::sync::CancellationToken,
    pub progress: Arc<UnitProgress>,
    pub layout: Layout,                  // this kind's layout
    pub index: DownloadIndex,            // read only for the "already on disk" short cut
}
```

The actor, not the runner, does what both runners did separately: the free-space precheck (`Layout::entry_dir` creates the
entry folder, then `check_free_space`), setting `Downloading`, the index update on success, the event emission.
`manga::pipeline::run_chapter` and `anime::downloader::EpisodeEngine::run` keep their bodies; only the context type and the
return value change. `ChapterPipeline` holds the page cache, the splitter settings, the speed limiter, an
`Arc<dyn ChapterSources>` to find the live source, and the per-unit page-list map (B37). `EpisodeEngine` keeps its
resolver, fetch, ffmpeg paths and drops `unique_episode_dir_name` (B12).

```rust
pub trait ChapterSources: Send + Sync { fn chapter_source(&self, source_id: i64) -> Option<Arc<dyn ChapterSource>>; }   // was delete::ChapterSourceResolver
pub trait SourceDirectory: Send + Sync {
    /// The source of an id as the downloads need it: live source, else the stub table's name, else `None`.
    fn source_key(&self, kind: Kind, source_id: i64) -> Option<SourceKey>;
}
```

`SourceDirectory` is the one place the app answers "what is this source called" (today `anime_queue::source_name` and
`LiveRegistry.name_of_known`). Restore and the pending flush both use it.

### 2.4 The lane state machine (`next/state.rs`, was `queue.rs`)

A pure type with no I/O; one lane per kind. The actor owns `Lanes { manga: Lane, anime: Lane }` and picks one with
`fn of(&mut self, kind: Kind) -> &mut Lane`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState { Stopped, Running, Paused }

pub enum StartMode { Explicit, Resume(HashSet<i64>) }   // Explicit retries ERROR; Resume retries only the given ids (network-stopped)

pub struct Lane { /* items: Vec<Arc<QueuedUnit>>, run: RunState */ }
impl Lane {
    pub fn new(kind: Kind) -> Self;
    pub fn restored(kind: Kind, units: Vec<QueuedUnit>) -> Self;
    pub fn items(&self) -> &[Arc<QueuedUnit>];
    pub fn get(&self, unit_id: i64) -> Option<&Arc<QueuedUnit>>;
    pub fn run_state(&self) -> RunState;
    pub fn add_all(&mut self, units: Vec<QueuedUnit>) -> Vec<i64>;         // skips queued and repeated ids
    pub fn add_front(&mut self, units: Vec<QueuedUnit>) -> Vec<i64>;
    pub fn remove_units(&mut self, ids: &[i64]) -> Vec<Arc<QueuedUnit>>;
    pub fn remove_entry(&mut self, entry_id: i64) -> Vec<Arc<QueuedUnit>>;
    pub fn clear(&mut self) -> Vec<Arc<QueuedUnit>>;
    pub fn reorder(&mut self, ordered: &[i64]) -> bool;                    // false when the order does not change (B23)
    pub fn set_state(&mut self, unit_id: i64, to: DownloadState) -> Result<bool, QueueError>;
    pub fn set_waiting(&mut self, unit_id: i64, waiting: bool) -> bool;
    pub fn begin(&mut self, mode: StartMode) -> Vec<i64>;                  // returns the ids put back to QUEUE
    pub fn pause_running(&mut self) -> Vec<i64>;                           // DOWNLOADING -> QUEUE, run = Paused
    pub fn stop_running(&mut self) -> Vec<i64>;                            // DOWNLOADING -> ERROR, run = Stopped
    pub fn requeue_running(&mut self) -> Vec<i64>;                         // for mutations: DOWNLOADING -> QUEUE, run unchanged
    pub fn pick(&self, max_sources: usize) -> Vec<i64>;                    // scheduler::pick_active over non-waiting items
    pub fn finish(&mut self, unit_id: i64) -> Option<Arc<QueuedUnit>>;     // removes a downloaded unit
    pub fn all_idle(&self) -> bool;
}
```

Every one of the 17 `queue.rs` tests ports with the item type changed. The anime actor gets the same rules for free.

### 2.5 The actor and its handle (`next/actor.rs`, `next/queue.rs`)

One actor task owns `Lanes`, the `JoinSet` of running units, the store handle, the gate, and the progress flush. Commands arrive on
an **unbounded** channel (B: the manga channel dropped on a full queue). Every store write, every `Layout` call and every
`index` mutation that touches disk runs in `spawn_blocking`; the actor awaits it.

```rust
pub struct QueueParams {
    pub db: Arc<Db>,                                    // kuro_db::Db: the stores use read/write on the one handle
    pub store: Arc<QueueStore>,
    pub runners: Vec<Arc<dyn UnitRunner>>,              // one per kind
    pub directory: Arc<dyn SourceDirectory>,
    pub layouts: Layouts,
    pub index: DownloadIndex,
    pub free_space: FreeSpaceFn,
    pub max_active: Arc<dyn Fn() -> usize + Send + Sync>,   // the "Maximum downloads" pref, read at every scheduling pass
    pub connectivity: watch::Receiver<Connectivity>,
    pub only_over_wifi: bool,                           // false: the diet cut the switch
}
pub struct Layouts { pub manga: Layout, pub anime: Layout }
impl Layouts { pub fn of(&self, kind: Kind) -> &Layout; }

pub struct EnqueueRequest {
    pub kind: Kind, pub entry: EntryRef, pub source: SourceKey, pub units: Vec<UnitRef>,
    pub place: Place, pub auto_start: bool,
}
pub enum Place { Back, Front }
pub struct Enqueued {
    pub added: Vec<i64>, pub started: bool, pub warning: Option<BulkWarning>,
    pub continue_toast: bool, pub refused: Option<Refusal>,
}
pub enum Refusal { LocalSource, NothingToDo }
pub struct BulkWarning { pub text: &'static str, pub timeout_secs: u32, pub help_url: &'static str }
pub enum CancelTarget { Units(Vec<i64>), Entry(i64) }
pub struct Snapshot { pub items: Vec<Arc<QueuedUnit>>, pub manga: RunState, pub anime: RunState }
impl Snapshot {
    pub fn of(&self, kind: Kind) -> impl Iterator<Item = &Arc<QueuedUnit>>;
    pub fn run(&self, kind: Kind) -> RunState;
    pub fn find(&self, kind: Kind, unit_id: i64) -> Option<&Arc<QueuedUnit>>;
}

#[derive(Clone)]
pub struct Queue { /* tx: mpsc::UnboundedSender<Command>, snapshot: watch::Receiver<Arc<Snapshot>>, events: broadcast::Sender<DownloadEvent> */ }
impl Queue {
    pub fn spawn(params: QueueParams) -> Queue;                         // inside a tokio runtime; restores and sweeps first (2.8)
    pub async fn enqueue(&self, req: EnqueueRequest) -> Result<Enqueued, DownloadError>;
    pub async fn start_now(&self, req: EnqueueRequest) -> Result<(), DownloadError>;   // front + start; AlreadyDownloaded when on disk (B25)
    pub fn start(&self, kind: Kind);                                    // explicit start (retries ERROR)
    pub fn pause(&self, kind: Kind);
    pub fn clear(&self, kind: Kind);
    pub fn reorder(&self, kind: Kind, unit_ids: Vec<i64>);
    pub async fn cancel(&self, kind: Kind, target: CancelTarget) -> Result<Vec<i64>, DownloadError>;
    pub async fn hold_entry(&self, kind: Kind, entry_id: i64) -> Result<EntryHold, DownloadError>;   // pauses a run that holds this entry; drop resumes (B32)
    pub fn sources_changed(&self);                                      // re-evaluates every waiting flag and source name
    pub fn snapshot(&self) -> Arc<Snapshot>;
    pub fn subscribe(&self) -> watch::Receiver<Arc<Snapshot>>;
    pub fn queued(&self, kind: Kind, unit_id: i64) -> Option<Arc<QueuedUnit>>;
    pub fn events(&self) -> broadcast::Receiver<DownloadEvent>;
}
```

Actor rules (each one is a test in section 4):

- Scheduling: `lane.pick((params.max_active)().max(1))`; a running unit whose id leaves the picked set is aborted and goes to QUEUE.
- Start: `StartMode::Explicit` from `Queue::start`; `StartMode::Resume(network_failed)` after the network returns.
- Pause: aborts running units (they go to QUEUE) and ends the lane's run as `Paused`; clears that lane's resume-on-network flag.
- Mutation (remove, reorder, delete through `hold_entry`): `requeue_running` then, when the lane was running, restart with `StartMode::Resume(empty)` so no ERROR unit is retried (B16).
- Progress: when a lane has running units the actor owns a 100 ms interval; for each running unit whose `progress.version()` moved it emits one `DownloadEvent::Progress`. No timer runs while nothing is running. This gives the 10 Hz throttle of step 6 for free.
- Finished: `index.add_unit(kind, source.id, source.dir, entry.title, dir_name)`, `lane.finish`, save, `Status`, `Finished`, `QueueChanged`.
- Error: `FailureKind::Network` units join the network-failed set; `InvalidLocation` stops the lane (B17); every other error is one `DownloadEvent::Error`.
- Persist: after every membership or order change, `QueueStore::replace(kind, rows)` in `spawn_blocking`.

### 2.6 Gate (`next/gate.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Connectivity { pub online: bool, pub metered: bool }

/// Two-second debounce (anime's `NetworkGate`), applied per lane. A lane stopped by the network starts again by itself
/// when the network is back, unless the user paused it meanwhile.
pub struct Gate { /* blocked_since: Option<Instant>, stopped_by_network: [bool; 2] */ }
impl Gate {
    pub fn observe(&mut self, now: Instant, c: Connectivity, only_over_wifi: bool, running: [bool; 2]) -> [GateAction; 2];
    pub fn user_paused(&mut self, kind: Kind);
}
pub enum GateAction { Stop(&'static str), Resume, Nothing }
pub fn verdict(c: Connectivity, only_over_wifi: bool) -> Option<&'static str>;   // the existing texts
```

`anime::network::{NetworkProbe, SysfsNetwork, read_sysfs}` have no caller in the app and read `/sys` from a crate that must not
hold OS code. They are deleted in phase D (the app feeds `Connectivity` from `gio::NetworkMonitor`, which `browse/errors.rs:83`
already uses for a one-off check). `ProcessHold` and `NoHold` are deleted too if decision D4 is taken as recommended.

### 2.7 Stores (`next/store.rs`, `next/pending.rs`, `kuro-db`)

One SQL module `kuro_db::tables::downloads` (new, tested against `schema_unified::open_in_memory()`, the step 4 convention), so
no raw SQL stays in `kuro-downloads`. The stores sit on `Arc<Db>` (`Db::read`, `Db::write`, `write_async`), so there are no extra
SQLite connections: today `downloads::start` opens two more connections per kind through `open_in_data_dir`.

```rust
pub struct StoredRow { pub unit_id: i64, pub entry_id: i64, pub source: i64, pub position: i64, pub state: u8, pub enqueued_at: i64 }

pub struct QueueStore { /* db: Arc<Db>, unreadable: [AtomicBool; 2] */ }
impl QueueStore {
    pub fn new(db: Arc<Db>) -> Self;
    pub fn load(&self, kind: Kind) -> Result<Vec<StoredRow>, StoreError>;          // ORDER BY position; sets the guard on failure
    pub fn replace(&self, kind: Kind, rows: &[StoredRow]) -> Result<(), StoreError>; // StoreError::Unreadable while the guard is set (B4)
}
```

`replace` inserts with `INSERT ... SELECT ... FROM items i JOIN entries e ON e._id = i.entry_id WHERE i._id = ?` (anime's current
`COALESCE((SELECT source ...))` pattern, but the join also drops a unit whose item was deleted). A plain insert would break the
whole transaction on the first foreign-key miss (`download_queue.unit_id REFERENCES items ON DELETE CASCADE`, and
`(entry_id, kind) REFERENCES entries`), which would stop the queue from being saved at all.

```rust
pub struct PendingGroup { pub kind: Kind, pub entry: EntryRef, pub units: Vec<UnitRef> }
pub struct PendingStore { /* db: Arc<Db> */ }
impl PendingStore {
    pub fn new(db: Arc<Db>) -> Self;
    pub fn add(&self, kind: Kind, entry: &EntryRef, units: &[UnitRef]) -> Result<usize, StoreError>;   // dedup by unit id, per-entry seq
    /// Takes the rows whose source `known` accepts; the rest stay (E-P01). Rows that cannot be decoded go to
    /// `download_pending_corrupt` and are removed (B28).
    pub fn take(&self, known: &dyn Fn(Kind, i64) -> bool) -> Result<Vec<PendingGroup>, StoreError>;
    pub fn put_back(&self, group: &PendingGroup) -> Result<usize, StoreError>;                          // B29
    pub fn len(&self) -> Result<usize, StoreError>;
}
```

### 2.8 Restore (`next/restore.rs`)

`Queue::spawn` restores before it serves a command: `store.load(kind)`, then in one `spawn_blocking` it reads `EntryRepo::by_id`
and `ItemRepo::by_id` for each row, `directory.source_key(kind, source)` for the source, and `runner.can_run`. Rows whose item or
entry is gone are dropped (the foreign key already removed them from the table). A row whose source cannot be named at all is kept:
it becomes a `QueuedUnit` with `source.availability = Stub`, `waiting = true`. After the restore the actor sweeps staging with
`layout.sweep_staging(keep)` where `keep` is every restored unit's staging folders (fixes B20). `Queue::sources_changed` re-asks
`directory` and `can_run` for every item and flips `waiting`; a unit that stops waiting is picked by the next scheduling pass if
the lane runs. The app's `spawn_restore`, `Waiting`, `RestorePass`, `cancel_waiting` and the `waiting` list on `Queues` all go:
a waiting row is an ordinary queued unit, so Cancel on the queue page is the ordinary cancel.

### 2.9 Layout (`next/layout.rs`, `next/naming.rs`)

One `Layout` per kind (two roots stay: `StorageManager::downloads_dir(kind)`). The root is required (anime's `Option` goes).

```rust
#[derive(Clone)]
pub struct Layout { /* kind: Kind, root: PathBuf */ }
impl Layout {
    pub fn new(kind: Kind, root: impl Into<PathBuf>) -> Self;
    pub fn kind(&self) -> Kind;
    pub fn root(&self) -> &Path;
    pub fn source_dir_name(source: &SourceKey) -> String;                  // one function for the app and the orphan scan (B35)
    pub fn entry_dir_name(title: &str) -> String;
    pub fn unit_dir_names(&self, unit: &UnitRef) -> [String; 2];           // current name, then the legacy one (kind rule in naming.rs)
    pub fn entry_dir(&self, source: &SourceKey, title: &str) -> Result<PathBuf, DownloadError>;   // creates
    pub fn find_source_dir(&self, source: &SourceKey) -> Option<PathBuf>;
    pub fn find_entry_dir(&self, source: &SourceKey, title: &str) -> Option<PathBuf>;
    pub fn find_unit(&self, source: &SourceKey, title: &str, unit: &UnitRef) -> Option<PathBuf>;
    pub fn find_units(&self, source: &SourceKey, title: &str, units: &[UnitRef]) -> (Option<PathBuf>, Vec<PathBuf>, Vec<PathBuf>); // (entry dir, finished, staging)
    pub fn staging_dirs(&self, source: &SourceKey, title: &str, unit: &UnitRef) -> Vec<PathBuf>;
    pub fn rename_unit(&self, source: &SourceKey, title: &str, old: &UnitRef, new: &UnitRef) -> Result<Option<String>, DownloadError>;   // new dir name, None when not downloaded
    pub fn rename_source(&self, old: &SourceKey, new: &SourceKey) -> Result<Option<String>, DownloadError>;   // case-only two step, fails on an existing target
    pub fn prune_empty(&self, source: &SourceKey, title: &str) -> io::Result<Pruned>;   // entry folder, then source folder, when empty
    pub fn sweep_staging(&self, keep: &[PathBuf]) -> usize;
    pub fn remove_path(path: &Path) -> io::Result<()>;                     // NotFound is Ok (B10)
}
pub struct Pruned { pub entry_removed: bool, pub source_removed: bool }
pub fn check_free_space(free: Option<u64>) -> Result<(), DownloadError>;
```

`naming.rs` keeps `chapter_dir_name`, `valid_chapter_names`, `episode_dir_name`, `legacy_episode_dir_name`,
`valid_episode_dir_names`, `fat_safe` (which is the shared sanitiser, so it becomes a call to `sanitize::build_valid_filename`)
and the scan rule (`fn is_unit_entry(kind, name, is_dir) -> Option<String>`: manga folder or `.cbz` stem, anime folder or
`.mkv`/`.mp4` stem, both skipping `_tmp`). That is the whole "per-kind layout strategy".

### 2.10 Index (`next/index.rs`, `next/watch.rs`)

One `DownloadIndex`, owned by `Shell` (it already is for manga), holding both kinds. One snapshot file
(`download-index-v2.bin`, magic `KIDX\x02`) stores both trees and both roots; an old v1 or anime v3 file is simply ignored and
the next scan rebuilds it (the index is a cache).

```rust
#[derive(Clone)]
pub struct DownloadIndex { /* shared: Arc<Shared> */ }
pub struct IndexRoots { pub manga: PathBuf, pub anime: PathBuf }
impl DownloadIndex {
    pub fn open(roots: IndexRoots, cache_dir: &Path) -> Self;
    pub fn root(&self, kind: Kind) -> &Path;
    pub fn is_downloaded(&self, kind: Kind, source_id: i64, entry_title: &str, unit_name: &str, scanlator: Option<&str>) -> bool;
    pub fn unit_count_total(&self, kind: Kind) -> usize;
    pub fn unit_count_entry(&self, kind: Kind, source_id: i64, entry_title: &str) -> usize;
    pub async fn size_total(&self, kind: Kind) -> u64;
    pub async fn size_entry(&self, kind: Kind, source_id: i64, entry_title: &str) -> u64;
    pub fn entry_source_dir(&self, kind: Kind, source_id: i64, entry_title: &str) -> Option<String>;
    pub fn add_unit(&self, kind: Kind, source_id: i64, source_dir: &str, entry_title: &str, unit_dir: &str);
    pub fn remove_units(&self, kind: Kind, source_id: i64, entry_title: &str, unit_dirs: &[String]);
    pub fn rename_unit(&self, kind: Kind, source_id: i64, source_dir: &str, entry_title: &str, old_dir: &str, new_dir: &str);
    pub fn remove_entry(&self, kind: Kind, source_id: i64, entry_title: &str);
    pub fn remove_source(&self, kind: Kind, source_id: i64);
    pub fn rename_source_dir(&self, kind: Kind, source_id: i64, new_dir: &str);
    pub fn invalidate(&self, kind: Option<Kind>);                          // None: both (Settings > Reindex downloads, B: it only did manga)
    pub async fn renew_if_stale<F>(&self, kind: Kind, now_ms: i64, ready: watch::Receiver<bool>, installed: F) -> bool
        where F: FnOnce() -> Vec<(i64, String)> + Send;
    pub async fn rescan<F>(&self, kind: Kind, ready: watch::Receiver<bool>, installed: F) -> bool
        where F: FnOnce() -> Vec<(i64, String)> + Send;
    pub fn changes(&self) -> watch::Receiver<u64>;                         // anime's feature; the Library badge refreshes on it
    pub fn checking_indicator_visible(&self) -> bool;                      // manga's feature
    pub fn watch(&self) -> Option<RootWatch>;                              // one watcher over both roots; Shell holds the guard
}
```

The tree is `sources: BTreeMap<i64, SourceDir { dir_name, entries: BTreeMap<String, EntryDir { source_dir, dir_name, units: BTreeSet<String> }> }>`
per kind; reads clone an `Arc<Tree>` (anime's approach, so a listing never waits for a write). The local source count
(`local::chapter_count`, `ANIME_EPISODE_EXTENSIONS`) stays in `local.rs`; `Downloads::entry_usage` takes the local entry folder as
`Option<&Path>` from the caller, which knows whether the source is local.

### 2.11 Delete rules, locks, service (`next/delete.rs`, `next/locks.rs`, `next/service.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeleteRules { pub remove_bookmarked: bool }
impl DeleteRules { pub const EXPLICIT: Self = Self { remove_bookmarked: true }; pub const AFTER_READ: Self = Self { remove_bookmarked: false }; }
pub fn select_for_delete(units: Vec<UnitRef>, rules: DeleteRules) -> Vec<UnitRef>;
```

This assumes decision D3 (the category exclusions are cut, as the settings diet already cut their keys; today every caller passes
an empty exclude list). With D3 declined the struct keeps `exclude_categories` and `entry_categories` and `select_for_delete` is
the current function unchanged.

`SeriesLocks` moves to `next/locks.rs` unchanged, plus `pub fn lock_blocking(&self, kind: Kind, id: i64) -> OwnedMutexGuard<()>`
for the one synchronous caller (renames, 2.13).

`Downloads` is the one object the app talks to. It owns the `Queue`, the index, the layouts, the pending store, the locks, the
`SourceDirectory`, and the `Db`. Everything that touches disk runs in `spawn_blocking` under the series lock.

```rust
#[derive(Clone)]
pub struct Downloads { /* queue, index, layouts, pending, series: Arc<SeriesLocks>, directory, db */ }
impl Downloads {
    pub fn new(queue: Queue, index: DownloadIndex, layouts: Layouts, pending: Arc<PendingStore>, directory: Arc<dyn SourceDirectory>, db: Arc<Db>) -> Self;
    pub fn queue(&self) -> &Queue;
    pub fn index(&self) -> &DownloadIndex;

    pub fn is_downloaded(&self, entry: &EntryRef, source: &SourceKey, unit: &UnitRef) -> bool;           // index lookup
    pub fn locate(&self, entry: &EntryRef, source: &SourceKey, unit: &UnitRef) -> Option<PathBuf>;       // for reader / player (blocking: stat)
    pub async fn entry_usage(&self, kind: Kind, source_id: i64, title: &str, local: Option<&Path>) -> (u64, usize);

    /// Explicit or rule-filtered delete: units leave the queue, folders go, empty entry and source folders are pruned,
    /// the index follows. Returns how many units the rules selected.
    pub async fn delete_units(&self, entry: &EntryRef, source: &SourceKey, units: Vec<UnitRef>, rules: DeleteRules) -> Result<usize, DownloadError>;
    pub async fn delete_entry(&self, entry: &EntryRef, source: &SourceKey, remove_queued: bool) -> Result<(), DownloadError>;

    /// Delete-after-read: the rule runs now (`AFTER_READ`), the rows wait in `download_pending_delete`.
    pub fn defer_delete(&self, entry: &EntryRef, units: &[UnitRef]) -> Result<usize, DownloadError>;
    /// Runs when the reader or player closes, and once at startup. Reads the live bookmark flag of each unit from the
    /// database, drops rows of an entry that is gone (EC-93), puts a failed group back.
    pub async fn flush_pending_deletes(&self) -> Result<usize, DownloadError>;

    pub async fn rename_unit(&self, entry: &EntryRef, source: &SourceKey, old: &UnitRef, new: &UnitRef) -> Result<(), DownloadError>;
    pub fn rename_unit_blocking(&self, entry: &EntryRef, source: &SourceKey, old: &UnitRef, new: &UnitRef) -> Result<(), DownloadError>;
    pub async fn rename_source(&self, kind: Kind, old: &SourceKey, new: &SourceKey) -> Result<(), DownloadError>;
}
```

### 2.12 Events and notifier (`next/events.rs`, `next/notify.rs`)

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum DownloadEvent {
    Status   { kind: Kind, unit_id: i64, state: DownloadState },
    Progress { kind: Kind, unit_id: i64, entry_id: i64, entry_title: String, unit_name: String, done: u32, total: u32, percent: u8 },
    Started  { kind: Kind },
    Paused   { kind: Kind },
    Complete { kind: Kind },
    Dismiss  { kind: Kind },
    Warning  { kind: Kind, text: String, timeout_secs: Option<u32>, help_url: Option<String> },
    Error    { kind: Kind, unit_id: Option<i64>, entry_id: Option<i64>, entry_title: Option<String>, unit_name: Option<String>, message: String },
    Finished { kind: Kind, unit_id: i64, entry_id: i64 },
    QueueChanged { kind: Kind },
}
```

`DownloadNotifier::new(sink: Arc<dyn NotificationSink>, hide_content: Arc<AtomicBool>)` and
`dispatch(&mut self, now: Instant, event: &DownloadEvent)` replace `notify::dispatch`/`effect_for` and `AnimeNotifier`. Per kind it
keeps its own notification ids (`dl-manga-*`, `dl-anime-*`), its own `ErrorDigest` (B26) and its own done text ("Chapters are ready to
read offline", "Episodes are ready to watch offline"). It keeps what the app decided on 2026-10-12: no progress and no paused
notice, only the end and failures. `spawn_notifier(events, sink, hide)` is the only task; `anime_queue::notices` and its 1 s poll
are deleted.

### 2.13 App layer (`src/`)

- `src/downloads/mod.rs`: `start(...) -> Result<Started, String>` builds the `Queue` and `Downloads` once; `Started { downloads: Downloads, _watch: RootWatch, _network: NetworkFeed }`. `Queues` and `Waiting` are deleted.
- `src/downloads/platform.rs` (new, about 60 lines): `NetworkFeed` listens to `gio::NetworkMonitor` `network-changed` on the GTK thread and sends `Connectivity { online, metered }` into the `watch::Sender`. This is the missing feed (section 0 fact 1).
- `src/downloads/directory.rs` (new): `impl SourceDirectory` over the live sources and the stub table (the code of `anime_queue::source_name` and the `LiveRegistry.name_of_known` lookup that `misc/runner.rs` passes to `FolderRenames`, once), and `impl ChapterSources`.
- `src/downloads/{manga_io,anime_io}.rs`: keep `LiveChapterSource`, `LiveHosters`, `HttpVideoFetch`; lose `restore_rows`, `Waiting`, `test_manager` (replaced by a `testing` helper that builds a `Downloads` over an in-memory `Db`).
- `src/downloads/live.rs`: `queues()` becomes `downloads() -> Option<Downloads>`; `queued(kind, id)`, `cancel(kind, id)` and `follow(owner, on_change)` read `Queue::subscribe()` (a `watch` whose `changed()` can be awaited in a `glib::spawn_future_local`) instead of polling twice a second. `Entry::{Manga, Anime}` goes; the settled state of a unit that left the queue is `downloads.is_downloaded(...)` for both kinds.
- `src/downloads/queue_page.rs`: one `queue_model::from_snapshot(&Snapshot, kind)`; one `Pause`/`Resume` pair calling `Queue::{pause, start}`; the row menu calls `Queue::{cancel, reorder, start}`; a waiting row shows "Waiting for source" through `QueueItem.waiting`.
- `src/shell/host.rs`: `Shell::downloads()` stays and now returns the one `DownloadIndex`; `Shell::download_service() -> Option<Downloads>`; `start_downloads` calls `download_host::install(...)`.
- `src/updates/download_host.rs`: `anime_index()` and its `OnceLock` go. `IndexBackend` (the pre-install default) answers `on_disk`, `queue_entry` and `entry_usage` for both kinds from the one index; a new `LiveBackend` over `Downloads` is what `Shell::start_downloads` installs (the trait seam stays for the screens' fakes). The `external: bool` parameter of `enqueue` is dropped (the app always passes the diet constant).
- `src/reader/launch.rs`: `delete_sink` calls `downloads.defer_delete(...)` for each completed unit and `flush_pending_deletes()` on close (spawned on the runtime). `reader/delete_disk.rs`, `download_paths.rs` and the `QueueFile` half of `delete_queue.rs` are deleted; the in-memory `DeleteQueue` and `delete_target` slot rule stay.
- `src/library_update/renames.rs`: `FolderRenames` holds a `Downloads` (and the entry/source lookup it already has) and calls `rename_unit_blocking` for both kinds; the index follows for both (fixes drift row "episode rename never updates AnimeIndex").
- `src/entry_core/{backend.rs, downloaded.rs, anime/download_backend.rs}`: both kinds call `Downloads` (`enqueue`, `start_now`, `delete_units(.., DeleteRules::EXPLICIT)`, `is_downloaded`); `installed_queue`/`install_queue` and `AnimeDownloads` go. Step 6 later folds this into `DownloadPort`; here the edit stays minimal so step 6 sees one API.
- `library/{download_facts,mount}.rs`: both kinds read `index.unit_count_entry(kind, ...)` and `is_downloaded`; `browse/migrate_live.rs`: `has_downloads` and `delete_downloads` use the index and `Downloads::delete_entry` for both kinds (this also closes the "migration never deletes old anime downloads" row early; step 7 keeps the rest).
- `settings_ui/maintenance.rs`: Reindex calls `index.invalidate(None)`. `settings_ui/downloads.rs`: label "Maximum downloads". `shell/sidebar.rs`: `queued_percents` counts every lane through the snapshot, so waiting units are counted once, as items. `ext_host.rs`: `Layout::source_dir_name(&SourceKey)`. `misc/runner.rs`: `DownloadPrefs` from `kuro-domain` (the `kuro-downloads` copy and `settings.rs` are deleted).

---

## 3. Decisions needing the user

Short; each has a default, and I proceed on the default if you do not answer.

- **D1. Default for "Maximum downloads".** The pref `download_slots` defaults to 1 (clamped 1 to 5). Manga ran 5 sources at once regardless; anime followed the pref (default 1 in the app, 3 in the tests). Making the pref govern both would slow manga from 5 to 1 by default. **Recommend 3**, label "Maximum downloads" (the registry default changes with its test).
- **D2. One queue for both kinds, or a lane per kind?** The Downloads page has a Pause/Resume per kind today. **Recommend one actor with a lane per kind** (pause, run and limit per kind; one store, one gate, one notifier). The alternative, one shared lane, would make Pause pause both kinds.
- **D3. Cut the delete-category exclusion.** The settings diet already removed the keys, but `DeleteRules` still carries `exclude_categories` and the entry's categories, and every caller passes empty. Principle 1 says cut code, row, tests. **Recommend cut**: `DeleteRules { remove_bookmarked }`, delete-after-read keeps bookmarked units, an explicit delete removes them.
- **D4. Closing the app while a download runs.** Today the window closes, the process exits and the run dies silently; `ProcessHold` is passed `NoHold` and `lifecycle::CLOSE_PROMPT` ("Downloads will pause. Quit?") has no caller. **Recommend** a confirmation using that existing text when any lane runs, no background process, and `ProcessHold`/`NoHold` deleted. (A background hold needs a tray or a way to see and quit the hidden app, which is a UI decision I do not want to invent.)
- **D5. A restored row whose source has no name (not installed and no stub row).** Manga keeps it as Waiting, anime drops it. **Recommend keep as Waiting** for both, with Cancel on the queue page. Dropping a user's queued downloads because a source was removed is the worse failure.
- **D6. Fillermarked episodes.** Anime drops them at enqueue, also when the user picks one by hand, because the cut pref `download_fillermarked` is hard-coded off. **Recommend** the drop applies only to the automatic download after a refresh (a flag `skip_fillermarked` on `EnqueueRequest`); a hand pick always downloads.

Not asked, settled by the map: setting governs both kinds (1b item 1), thresholds 30/15 (2), replace on collision (3), download-ahead and the rest are other steps.

---

## 4. Phases (expand, then contract) and the test plan

Rules: the same as step 4. The workspace compiles at every commit. Lanes write code and tests only and do not run cargo (repo rule); the
lead builds once per merge. `scripts/check-path-literals.sh` and `scripts/check-platform-boundary.sh` run before a lane is called done
(`kuro-downloads` must not gain a `.join` on a `Paths` accessor or an OS word). New code is in `crates/kuro-downloads/src/next/`;
old modules are untouched until phase D.

### Phase A: shared types and the pure parts (new files only)

| Lane | Owns (only these files) | Seam it exports |
|---|---|---|
| A1 | `crates/kuro-downloads/src/next/{mod,unit,state,runner,gate,locks,delete,events}.rs`, `next/queue_model.rs` additions (`from_snapshot`, `waiting`), `crates/kuro-downloads/Cargo.toml` (adds `kuro-data`), `lib.rs` (one `pub mod next;` line) | 2.2, 2.3, 2.4, 2.6, 2.11 (rules, locks), 2.12 (events) |
| A2 | `crates/kuro-db/src/tables/{downloads.rs,mod.rs}`, `crates/kuro-db/src/notify.rs` only if the table list changes, `crates/kuro-testkit/src/db.rs` | `kuro_db::tables::downloads::{queue_load, queue_replace, pending_add, pending_take, pending_put_back, pending_corrupt}` |
| A3 | `crates/kuro-downloads/src/next/{layout,naming,index,watch}.rs` (merges `layout.rs`+`anime/layout.rs`+`anime/naming.rs`, `index.rs`+`anime/cache.rs`, `watch.rs`) | 2.9, 2.10 |

A1, A2, A3 are independent. Gate: the old tests still compile and pass; each lane's new tests are the ports in 4.1.

### Phase B: stores, actor, runners, service (still new files)

| Lane | Owns | Needs |
|---|---|---|
| B1 | `next/{store,pending,restore}.rs` | A1, A2 |
| B2 | `next/{actor,queue,notify}.rs` (actor, handle, `DownloadNotifier`, bulk warning, enqueue plan) | A1, A3 |
| B3 | `crates/kuro-downloads/src/manga/**` (moved copies of `pipeline.rs`, `images.rs`, `comicinfo.rs`, `source.rs`, `local.rs`, `build_page_list` from `manager.rs`, plus `runner.rs` = `ChapterPipeline`) and `anime/runner.rs` + the edits to `anime/downloader.rs` (impl `UnitRunner`, finalise replaces, drop `unique_episode_dir_name`) | A1, A3 |
| B4 | `next/service.rs` (`Downloads`) | A1, A3, B1 (trait shapes only; written against them) |

B3 copies rather than moves the manga files so the old crate API still compiles; phase D deletes the old copies.
Gate: `cargo test -p kuro-downloads -p kuro-db` green: old tests untouched, new tests in 4.1.

### Phase C: the app layer (callers). One seam commit first, then four parallel lanes, then the flip.

| Lane | Owns | What it does |
|---|---|---|
| C0 (one agent, first) | `src/downloads/{mod,platform,directory}.rs` (new `start`, `NetworkFeed`, `SourceDirectory`), `src/shell/host.rs` (starts the new `Downloads` beside the old stack with `persist = false`, adds `download_service()`), `src/updates/download_host.rs` (`LiveBackend`, `install` call, `IndexBackend` answers both kinds, `anime_index` removed) | a working `Downloads` for the lanes below; no caller moves yet |
| C1 | `src/entry_core/{backend,downloaded}.rs`, `src/entry_core/anime/download_backend.rs` | 2.13 entry-page bullet |
| C2 | `src/reader/**` (`launch.rs`, `delete_queue.rs`, `delete_disk.rs`, `download_paths.rs`, the screen's close hook) | delete-after-read through `defer_delete` + `flush_pending_deletes` |
| C3 | `src/library/**`, `src/misc/runner.rs`, `src/library_update/renames.rs`, `src/browse/migrate_live.rs` | facts, auto-download path, renames, migration cleanup |
| C4 | `src/settings_ui/{maintenance,downloads}.rs`, `src/ext_host.rs`, `src/recipe_sources.rs` (rescan hook for both kinds) | Reindex, label, orphan names |
| C5 (integrator, last) | `src/downloads/{live,queue_page,manga_io,anime_io,anime_queue}.rs`, `src/shell/sidebar.rs`, `src/screens.rs` (one line) | the flip: `persist = true`, the old stack and `Queues` are removed from `Shell`, `live`/`queue_page`/`sidebar` read the snapshot |

Merge order: C0, then C1 to C4 in any order (each only reads `Downloads`), then C5. Between C0 and C5 the app runs two stacks and the
new one does not persist, so a half-merged train is never pushed; the integrator pushes only after C5 and the full suite. If C1 to
C4 collide with the step 6 edits of `entry_core`, the step 6 side wins and the C lane redoes its small edit (the step 4 rule).
Contingency: if C0's "two stacks side by side" proves awkward (both write `download_queue`), C0 stops persisting in the old stack
instead of the new one; do not let both write.

### Phase D: contract

| Lane | Owns | What it does |
|---|---|---|
| D1 (one agent) | `crates/kuro-downloads/**` | delete the old modules (`downloader.rs`, `manager.rs`, `queue.rs`, `engine.rs`, old `store/pending/layout/index/notify/watch.rs`, `model.rs` except `DownloadState`, `anime/{manager,store,pending,cache,layout,notify,network,lifecycle,delete_rules,enqueue}.rs`, `download_prefs.rs`, `settings.rs`, `delete.rs` old half), `git mv next/* ..`, fix `use` paths, update `README.md` and `lib.rs`, delete the old fixtures that only the old stores read |
| D2 | `crates/kuro-domain/src/download/{policy,prefs}.rs`, `crates/kuro-domain/src/app/delete_download.rs` if `rg` shows no user (`ItemDownloads` is implemented in `src/library/actions.rs`, so check before deleting), `crates/kuro-prefs` registry default and label for D1 | one `DownloadPrefs`, one pair of thresholds |
| D3 (integrator) | the whole workspace | `rg` gates (4.3), full suite, real flows (4.4) |

Out: about 8,500 lines including their tests (old actors, facades, stores, index and notify copies, the reader delete path);
in: about 5,000 including tests. Net about 3,500, in line with the map's 3,300.

### 4.1 Existing tests that are ported (assertions kept, setup changes)

| Old tests | Count | Becomes | Change |
|---|---|---|---|
| `queue.rs` | 17 | `next/state.rs` | item type only |
| `manager.rs` (queue, enqueue, pause, network, delete, rename, flush, locks) | 43 | `next/queue.rs`, `next/service.rs` | run for `Kind::Manga` and `Kind::Anime` through a `FakeRunner` (the anime `FakeRunner` pattern); manga page-level tests stay in `manga/` |
| `downloader.rs` | 7 | `next/actor.rs` | hold and race tests run for both kinds |
| `anime/manager.rs` | 34 | `next/queue.rs`, `next/service.rs` | merged with the manga cases of the same rule (enqueue filter, start-now, reorder, remove, delete, rename, flush) |
| `engine.rs` | 10 | `next/queue.rs` | the reorder, remove, clear and start/stop cases through `Queue` |
| `pipeline.rs`, `images.rs`, `comicinfo.rs` | 19 + 16 + 5 | `manga/` | unchanged except the context type |
| `anime/downloader.rs`, `ffmpeg.rs`, `resolve.rs`, `local_server.rs`, `torrent.rs`, `offline.rs`, `progress.rs` | 18 + 23 + 4 + 7 + 3 + 6 + 3 | `anime/` | unchanged; the collision test (`E-F03`) flips to "replaced" (decision 3 of 1b) |
| `store.rs`, `anime/store.rs` | 8 + 4 | `next/store.rs` | one parametric set over both kinds, plus the guard test |
| `pending.rs`, `anime/pending.rs` | 4 + 3 | `next/pending.rs` | one set; the E-P01, E-P04 and group-restore cases run for both kinds |
| `index.rs`, `anime/cache.rs` | 13 + 16 | `next/index.rs` | one set over both kinds; the snapshot round trip uses the v2 file |
| `layout.rs`, `anime/layout.rs`, `anime/naming.rs`, `sanitize.rs` | 7 + 14 + 11 + 3 | `next/layout.rs`, `next/naming.rs` | same cases through `Layout::new(kind, root)` |
| `watch.rs` | 2 | `next/watch.rs` | one watcher, two roots |
| `notify.rs`, `anime/notify.rs` | 8 + 12 | `next/notify.rs` | the text and digest cases per kind |
| `anime/enqueue.rs`, `anime/network.rs`, `delete.rs`, `anime/delete_rules.rs` | 8 + 9 + 4 + 4 | `next/{queue,gate,delete}.rs` | thresholds read 30/15; `delete` per D3 |
| `src/downloads/anime_queue.rs` | 5 | `src/downloads/directory.rs` and `next/restore.rs` | source-name and stub cases; "row dropped when the source name is unknown" flips to "row kept, waiting" (D5) |
| `src/reader/delete_disk.rs` | 5 | `next/service.rs` (`flush_pending_deletes`) | the five cases (queued chapter deleted, `.cbz`, bookmark kept, local and ambiguous never touched, missing counts as dealt with) run through the pending table |
| `src/reader/delete_queue.rs` | 6 | `src/reader/delete_queue.rs` | the slot-rule tests stay; the `QueueFile` tests are deleted |
| `src/updates/download_host.rs` | 5 | same file | the default-backend test now expects anime `on_disk` to work |
| `src/library_update/renames.rs` | 6 | same file | one case per kind asserts the index changed |
| `src/entry_core/backend.rs` (`test_manager`) | 1 big | same file | builds a `Downloads` through the new test helper |

### 4.2 New tests that prove the drift fixes (each runs for both kinds unless noted)

1. **Pause, resume, reorder.** Enqueue 3 units, `start`, `pause`: running units are back in QUEUE and the lane is `Paused`; `start` completes all 3 and emits one `Complete { kind }`. `reorder([1, 99])` ends `[1, 3, 2]` and restarts a running lane; a reorder that changes nothing does not restart (B23); an empty reorder never clears.
2. **Mutation does not retry errors (B16).** One unit failed, `cancel` of another: the failed unit is still ERROR.
3. **Restart with a missing source.** Rows in `download_queue` for units whose source `directory` cannot name (anime and manga): after `Queue::spawn` they are queued items with `waiting = true`, never run, and `QueueStore::replace` keeps them; `sources_changed` after the source appears runs them. A row whose item was deleted is gone (D5).
4. **Staging survives a restart (B20).** A restored manga unit's `_tmp` folder is still on disk after `Queue::spawn`; an unrelated `_tmp` is swept.
5. **Network off stops both.** Send `Connectivity { online: false, .. }` into the watch after 2 s of virtual time (`tokio::time::pause`): both lanes `Stopped`, one `Warning { kind }` each, running units in ERROR; online again: only the lanes the network stopped restart, a lane the user paused stays paused. `InvalidLocation` stops the lane for both kinds (B17).
6. **Unreadable store (B4).** Corrupt the table so `load(Anime)` fails: `replace(Anime, ..)` returns `Unreadable` and the rows stay; the manga lane still saves.
7. **Full events for anime (B13).** An anime unit emits `Started`, `Status(Downloading)`, `Progress`, `Status(Downloaded)`, `Finished`, `Complete`.
8. **Max active from the pref for both kinds (B1).** Three sources, setting 2: two run at once, for each kind; a change applies on the next pass.
9. **Delete after read.** `defer_delete` then `flush_pending_deletes` for both kinds: files, index and pending rows are gone; a bookmarked unit stays with `AFTER_READ` and goes with `EXPLICIT` (B31); a second flush after the folder was removed by hand succeeds (B10); a row of an unnamed source stays; a failed group is put back unchanged; a corrupt row lands in `download_pending_corrupt`. Reader test (`src/reader`): closing the reader calls the flush once (a recording `Downloads` helper).
10. **Rename updates the index (both kinds).** `rename_unit` then `index.is_downloaded(new)` is true and `(old)` false with no rescan; a target that exists fails and keeps the old; a unit that is not downloaded is `Ok(None)`. `FolderRenames` test over a real `Downloads` for an anime episode.
11. **One index.** Both trees round trip through one snapshot file; the watcher invalidates only the kind whose root changed; `invalidate(None)` marks both stale.
12. **Facts and backend.** Library `download_facts::count(Kind::Anime, ..)` returns the episode count; `IndexBackend::on_disk(Kind::Anime, ..)` is true for a downloaded episode; `Shell::start_downloads` leaves `download_host::current()` an installed `LiveBackend` (not the default); an Updates-screen enqueue reaches the queue.
13. **Commands are never dropped (old manga `try_send(64)`).** Post 1,000 commands without awaiting; the queue ends in the expected state.
14. **Collision replaces (B12).** An anime episode folder that already exists is replaced and the unit is `Downloaded`.
15. **Queue page.** One `from_snapshot` test: both kinds list their own lane, a waiting unit has `waiting = true` and the "Waiting for source" line.

### 4.3 Gates

- `cargo test --workspace --no-fail-fast` green; `bash scripts/check-path-literals.sh && bash scripts/check-platform-boundary.sh .` green.
- `rg 'DownloadManager|AnimeQueue|AnimeIndex|AnimeLayout|DownloadLayout|MangaEngine|AnimeEngine|QueuedDownload|AnimeDownload|ChapterRef|MangaRef|EpisodeRef|AnimeRef|MangaPendingStore|AnimePendingStore|AnimeQueueStore|QueueRow|Queues\b|Waiting\b|DiskDeleter|QueueFile|anime_index' src crates` returns nothing outside comments.
- `rg 'ANIME_MAX_ACTIVE_SOURCES|MAX_ACTIVE_SOURCES|METERED_TOTAL_THRESHOLD|NoHold|ProcessHold'` returns nothing (per D4).
- `rg 'kind == Kind::(Manga|Anime)|Kind::(Manga|Anime) =>' src/downloads src/shell/sidebar.rs src/updates/download_host.rs` returns nothing except in `manga_io.rs`, `anime_io.rs` and `naming.rs`.

### 4.4 Real flows on the test display (end of D3)

Queue 3 chapters and 2 episodes, pause, resume, reorder, cancel one; quit mid-download and restart (partial pages kept, an episode of a removed source shows Waiting); network off (Settings > airplane mode or `nmcli networking off`) stops both and the return resumes; read a chapter with delete-after-read set and close the reader; the Library shows the anime downloaded badge and the downloaded-only filter works; Updates and Storage show anime size; Reindex downloads; rename an episode at a library refresh and open it from the title page; Delete downloaded on the title page for a bookmarked episode.

---

## 5. Risks and what I am unsure about

- **Two stacks during phase C.** C0 runs the new `Downloads` next to the old stack until C5 flips. Both write `download_queue`; the contingency in phase C says which one must not persist in the meantime. If this is too clumsy, collapse C into one sequential agent. It is the largest risk of the step.
- **`download_queue` foreign keys.** The table cascades from `items` and `entries`. A unit whose item is deleted while it is queued disappears from the table but stays in memory (it will still download). Reconcile on the `Db` change signal is not designed here; the risk is cosmetic (a download for a deleted item), not data loss. The insert-by-join in 2.7 is what keeps one stale unit from failing the whole save.
- **`Db` as the store handle.** I read `runtime.rs` for `read`/`write`/`write_async` and `notify.rs` (it lists the download tables, so writes ping live-query subscribers). I did not check that no view or subscription depends on `download_queue`; if one does, every save wakes it. Check in phase A2.
- **Gio objects are not `Send`.** `NetworkFeed` must forward `network-changed` from the GTK thread into a `watch::Sender` (which is `Send`) without holding a GObject in the actor. The same constraint is why D4 recommends deleting `ProcessHold`: a `gio::Application::hold` guard would need a thread-local trampoline.
- **`rename_unit_blocking` thread.** `FolderRenames::rename` is a synchronous port (`ItemRenames`) called from `SyncItems`. `SeriesLocks::lock_blocking` uses tokio's `blocking_lock`, which panics on a runtime worker. I did not trace which thread the sync call runs on (it looks like `spawn_blocking`, which is fine). If it runs on an async worker, make the port async instead.
- **Dropping the shared `pages` list.** B37 keeps the page list in a map inside `ChapterPipeline`, so a pause and resume within one session still skips the page-list request. After a restart it is fetched again (as today).
- **Fillermarked rule.** The current behaviour may be intended (Aniyomi's default skips fillermarked episodes for automatic downloads); I could not confirm whether it applies to a manual pick in Aniyomi. D6 is my reading.
- **Unknown-name rows and the sweep.** A Waiting row whose source cannot be named has no folder path, so its staging folder cannot be kept by the sweep. It is swept; a resume starts that unit over.
- **Entry projections.** `EntryRef`/`UnitRef` duplicate a few field names of `Entry`/`Item`. I chose them over `Arc<Entry>` because pending and Waiting rows do not have a whole row. If you prefer the full types in the queue, only `unit.rs` and the two `From` impls change.
- **Step 6 and step 7 contention.** `entry_core` is rewritten in step 6; C1 keeps its edit to the download calls. `browse/migrate_live.rs` belongs to step 7; C3 touches only its two download methods.
- **Dead code I found and did not delete here** (step 12 candidates): `throttle.rs` `SpeedLimiter` (always unlimited), `PageImageCache` (the app passes `None`), `anime/ffmpeg.rs::startup_notice`/`FfmpegNotice` (no reader), `anime/lifecycle.rs::close_needs_confirmation` (becomes live under D4), `kuro_domain::app::delete_download` (check users).
- **`crates/kuro-downloads/testdata` and `tests/fixtures`.** `testdata_tests.rs` (6 tests) reads fixtures written by the old stores' schema. I assumed they regenerate with `make_fixtures.py`; I did not open them.
