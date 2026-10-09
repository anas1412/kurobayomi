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
| `layout` | folder names and lookups (never creates folders, except `manga_dir`) |
| `sanitize` | `build_valid_filename` port and the Windows reserved-name rule |
| `store` | SQLite queue table, written whole in one transaction |
| `notify` | pure event-to-notice mapping and the `NotificationSink` trait |
| `events` | `DownloadEvent` (broadcast, capacity 64) |

## Decisions

- **`ntfs-names` (default on), D-DL-01.** A reserved device name gets `_` appended before its extension
  (`CON` becomes `CON_`, `LPT1.txt` becomes `LPT1_.txt`), so folders copied to a Windows disk stay valid. Titles
  without a reserved name produce the Kotlin output byte for byte.
- **Queue table DDL** lives in `sql/download_queue.sql` and is applied idempotently by `QueueStore::open`. Area 03
  may move it into its numbered migrations.
- **Index snapshot** is `KIDX\x01` followed by JSON, in `download-index-v1.bin`. bincode is not in the local
  registry cache, and JSON keeps the format inspectable. A file that fails to parse, or was written for another
  root, is deleted.
- **Free space** is injected as a `fn(&Path) -> Option<u64>`. The GUI passes
  `kuro_platform::linux::storage::free_bytes`.
