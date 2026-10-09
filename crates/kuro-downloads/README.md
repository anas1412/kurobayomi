# kuro-downloads

Manga download queue, per-source scheduler, page download, integrity check and the shared download index
(Aniyomi areas 23 to 25). No GTK and no OS-specific code: free disk space, network state, the process hold and
notification delivery are injected by the GUI crate.

## Layout

| Module | Role |
|---|---|
| `manager` | `DownloadManager` facade and `ManagerParams` |
| `downloader` | actor: owns the queue, runs the scheduler, gates on connectivity |
| `queue` | pure state machine with the status transitions |
| `scheduler` | `pick_active`: at most 5 sources, one chapter each, ERROR skipped |
| `pipeline` | one chapter: precheck, tmp folder, page list, fetch, integrity, finalise |
| `images` | page file names, extension resolution, resume lookup, integrity count |
| `index` | source, manga and chapter tree: scan, hourly renew, debounced snapshot |
| `layout` | folder names and lookups (never creates folders, except `manga_dir`); the startup sweep of stray `_tmp` folders and `.cbz.part` files (T-23a-044) |
| `data_saver` | data-saver proxy for page requests: Bandwidth Hero and wsrv.nl, off by default (T-23a-017) |
| `queue_model` | pure queue model of the Download page: sections, stable sort and reverse, moves, series moves, removals (T-25a-020, T-25a-031) |
| `page` | pure rules of the Download page: FAB, sort menu, row menu, row text and progress phase, empty state, keyboard; `RowProgressGate` (one repaint per 50 ms per row) and `poll_for_pages` (600 ticks of 50 ms, then a warning) for the row loops (T-25a-021 to T-25a-030) |
| `sanitize` | `build_valid_filename` port and the Windows reserved-name rule |
| `store` | SQLite queue table, written whole in one transaction; `manager::restore_queue` reads it back at startup and `ManagerParams::restored` keeps the staging folders of restored chapters through the startup sweep (T-23a-044) |
| `notify` | pure event-to-notice mapping and the `NotificationSink` trait |
| `events` | `DownloadEvent` (broadcast, capacity 64) |
| `view_model` | manga queue view: source groups, counts, row progress, `Msg` per event (T-23a-032, T-23a-034) |
| `throttle` | `ProgressThrottle`: one update per 50 ms per chapter (T-23a-034) |
| `retry` | shared 4-attempt schedule, 2 s, 4 s, 8 s, with cancellable waits (T-24a-015) |
| `anime::naming` | `fat_safe`, episode, legacy and source folder names (T-24a-004, T-24a-005) |
| `anime::layout` | anime folder lookups, creation, episode and source rename, free-space precheck (T-24a-006 to T-24a-008, T-24a-017) |
| `anime::progress` | `ProgressCell` (0..=100, publishes on change) and the ffmpeg percent rule (T-24a-002) |
| `anime::store` | SQLite `anime_download_queue`: add, remove, clear, restore (T-24a-009) |
| `anime::enqueue` | enqueue filter and order, queue-size warning (T-24a-012, T-24a-013) |
| `anime::resolve` | `HosterResolver` seam and `resolve_video` (T-24a-014) |
| `anime::manager` | queue actor, command channel, per-source scheduler with 3 sources, delete cascade (T-24a-010, T-24a-011, T-24a-034) |
| `anime::ffmpeg` | remux argv builder, ffprobe duration, progress parser, child driver with cancel (T-24a-018 to T-24a-021) |
| `anime::local_server` | `LocalHttpServer` seam, `localhost:1` URL rules, session that stops the server on drop (T-24a-022) |
| `anime::notify` | progress (1 Hz), paused, warning and error notices with an error digest (T-24a-026 to T-24a-029) |
| `anime::lifecycle` | `RunnerState` (stopped, running, paused) and the window-close rule (T-24a-030) |
| `anime::network` | Wi-Fi-only verdict, 2 s debounce, auto-resume gate, sysfs fallback probe (T-24a-031) |
| `anime::pending` | remove-after-watched SQLite store, deduplicated, installed-source flush (T-24a-032) |
| `anime::delete_rules` | bookmark and excluded-category filter for deletes (T-24a-033) |
| `anime::cache` | downloaded-episode index: scan, hourly renewal, debounced atomic snapshot, sizes (T-24a-035 to T-24a-037) |
| `anime::offline` | offline playback file choice and `file://` video (T-24a-039) |
| `anime::queue_view` | queue page rules: source groups, row text and bar, menu visibility and plans, drag rules (T-24a-040 to T-24a-043); sort helper, footer Stop/Continue, 200 ms rebuild gate (T-24a-044 to T-24a-046) |
| `anime::downloader` | `EpisodeEngine`, the `EpisodeRunner` for one episode: staging, resume, retry, success check, finalise with collision handling; startup `sweep_staging` (T-24a-016, T-24a-049, T-24a-062, T-24a-066) |
| `engine` | `QueueEngine` adapters for the two kinds (start, stop, pause_and_stop, clear, reorder, remove) (T-25a-017) |
| `settings` | download preference keys and defaults (`PREFERENCE_KEYS`, data section 3), speed limit KiB/s to bytes/s (T-24a-054, T-25a-002, T-25a-003, T-25a-005) |

## Decisions

- **New-item filter (T-25a-004) has one owner: `kuro-domain::download::filter`** (`should_download_new`, `filter_new`),
  which is also where `remove_after_read_index` lives. The copy that was here had no caller and is gone. Nothing
  enqueues new items automatically yet: the library-update flow (area 26) must call `filter_new` with the stored
  preferences and then `AnimeQueue::enqueue` or `DownloadManager::enqueue`. Until that area lands the flags have no
  effect on its own, which is the deferral.

- **Anime episode engine (T-24a-016) lives here.** Tasks T-24a-049, T-24a-062 and T-24a-066 depend on it, so
  `anime::downloader` implements the episode steps of behaviour 5.2 to 5.9 with the existing ffmpeg driver. Source
  headers are not passed to it: a resolved `Video` carries its own headers (`Video.headers`).
- **Finished folder name collision (E-F02, E-F03).** A new download whose plain folder name is held by another non-empty
  folder gets `<name> (<episode id>)`. Every lookup tries the plain, legacy and suffixed names. Finalise onto a
  non-empty folder is `EpisodeExists` and the staging folder is kept.
- **Network resume (E-N01).** `Command::Network` feeds `NetworkGate`. A run stopped by the network starts again when
  the network returns, unless the user paused or cleared the queue in between.
- **Pending deletes (E-P04).** A row that cannot be decoded moves to `anime_download_pending_corrupt`. A group whose
  delete fails goes back into the pending table.
- **`download_slots` (E-12, E-PR01, T-25a-003).** Decision: hidden. The key stays in `PREFERENCE_KEYS` (default 1) so a
  backup restores, but the engine ignores it, as Kotlin does, and `settings::DOWNLOAD_SLOTS_HIDDEN` is asserted by a
  test. The manga queue runs `scheduler::MAX_ACTIVE_SOURCES` sources at once. There is no replacement setting.
- **`download_speed_limit` (E-PR02).** Only the conversion is here (`speed_limit_bytes_per_sec`, KiB/s times 1024,
  0 is unlimited). No limiter is applied to anime downloads: ffmpeg copies streams and has no limiter here.
- **Preference keys (T-24a-054).** The Kotlin key names are used unchanged, so backups restore. The `downloads.*`
  registry paths in `downloader::SETTING_KEYS` are a separate list for the registry.
- **Test fixtures (T-24a-061).** The edge-cases section 5 trees are checked in under `tests/fixtures/dl/` (`tree_new`,
  `tree_legacy`, `tree_tmp`, `tree_case`). `testing::dl_fixture` copies one into a temp folder, so tests never change
  the checked-in files. Trees that a test shapes itself are still built in temp directories.
- **Startup ffmpeg check (T-24a-067).** `anime::manager::spawn` runs the check once (`ffmpeg::startup_notice`) and the
  handle keeps the result: `AnimeQueue::ffmpeg_notice()` is `Some` with the text "ffmpeg is not installed" and
  `FFMPEG_HELP_URL` when `ffmpeg` or `ffprobe` is missing. Showing it as a Settings row is the GUI crate's part. A
  missing binary during a run is `FfmpegMissing` and is not retried.
- **Cancel during the probe (T-24a-058).** `remux_once` races ffprobe against the cancel token (the dropped child is
  killed), and `run_remux` returns `Cancelled` before the spawn when the token is already cancelled, without touching
  the partial file: a newer run of the same episode may own it.
- **Filename limit (T-24a-072).** Only the file component (`<name>.mkv`, `<name>.tmp`) is cut to 200 UTF-8 bytes. Folder
  names keep the 240-unit rule of `fat_safe`.

- **`ntfs-names` (default on), D-DL-01.** A reserved device name gets `_` appended before its extension
  (`CON` becomes `CON_`, `LPT1.txt` becomes `LPT1_.txt`), so folders copied to a Windows disk stay valid. Titles
  without a reserved name produce the Kotlin output byte for byte.
- **Queue table DDL** lives in `sql/download_queue.sql` and is applied idempotently by `QueueStore::open`. Area 03
  may move it into its numbered migrations.
- **Index snapshot** is `KIDX\x01` followed by JSON, in `download-index-v1.bin`. bincode is not in the local
  registry cache, and JSON keeps the format inspectable. A file that fails to parse, or was written for another
  root, is deleted.
- **Settings.** `downloader::SETTING_KEYS` lists the keys this crate reads with their defaults. The Rust settings
  schema (area 02) registers them. `download_slots` is not among them: it is stored under its Kotlin key and hidden.
- **Tall split.** The optimal part height is injected (`DownloadSettings::optimal_image_height`, default 4096). The GUI
  sets it from GDK.
- **Anime queue.** Shares `DownloadState`, `scheduler::pick_active` and the retry schedule with the manga side. The
  runner and the hoster resolver are traits, implemented by the engine and the player areas.
- **Free space** is injected as a `fn(&Path) -> Option<u64>`. The GUI passes
  `kuro_platform::linux::storage::free_bytes`.
