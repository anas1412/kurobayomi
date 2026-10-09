# Area 18 — Topic: install pipeline, state machine, cancel and queue

Deep dive for downloads, install steps, cancel, uninstall and the queue. Full rules: `p1-behavior.md` section 7. Design: `p1-rust-design.md` section 9.5. Tasks: T-18a-033, T-18a-034, T-18a-035, T-18a-036, T-18a-037.

Repo-relative Kotlin paths are under `/home/blackbox/aniyomi-src`.

---

## 1. Kotlin pipeline in one picture

```
enqueue DownloadManager (I:93)  -> activeDownloads[pkg] = id, state[id] = Pending
  |
  |-- poll every 1 s (I:147): PENDING -> Pending, RUNNING -> Downloading, SUCCESS/FAILED stop polling
  |
  |-- ACTION_DOWNLOAD_COMPLETE broadcast (I:294):
  |       uri null          -> Error
  |       row found         -> installApk(id, uri)
  |                              LEGACY    -> activity, result -> Installed | Idle | Error
  |                              PRIVATE   -> copy to cache temp, installPrivateExtensionFile -> Installed | Error
  |                              PACKAGEINSTALLER/SHIZUKU -> service queue -> Installing -> Installed | Error | Idle
  |
  '-- merged flow stops at Installed | Error | Idle, then deleteDownload(pkg) (I:113-119)
```

Two facts follow from this shape:
- The poll never sets `Error`. A failed download becomes `Error` only through the broadcast (edge case I17).
- The flow's end is driven by the step, not by DownloadManager. A missing broadcast leaves the step at `Downloading` until the collector is cancelled.

---

## 2. Kurobayomi state machine

States are `InstallStep` (`Idle, Pending, Downloading, Installing, Installed, Error`). Terminal states are `Idle`, `Installed`, `Error` (behaviour 1.3).

```
            enqueue
  (none) ----------> Pending
                       |  permit acquired, first byte
                       v
                  Downloading ---- cancel ----> Idle (.part removed)
                       |                          ^
                       | body complete            |
                       v                          |
                  [verify digest, signature, fingerprint] -- fail --> Error (.part removed)
                       |                          ^
                       | ok                       |
                       v                          |
                  Installing  ---- check_install fail ---> Error
                       |
                       | atomic commit ok
                       v
                  Installed   (Installed event carries the reconciled record)
```

Rules:
- Cancel is accepted in `Pending`, `Downloading`, and `Installing` before the commit call starts. Once the commit starts it runs to completion (edge case I18).
- `Installed` and `Error` are emitted exactly once per job.
- Each emitted step is also the value of the row's state key, which is the package name (not the split row key).
- Transitions to `Installing` happen for every route, including the private route, which Kotlin skips (behaviour 7.6). Deliberate, so the UI shows the same states for every package.

### 2.1 Transition table

| From | Event | To | Side effects |
|---|---|---|---|
| (none) | enqueue | Pending | Job created, token created, key registered in `active` |
| Pending | permit acquired and first byte received | Downloading | Byte progress events start |
| Pending | cancel | Idle | Token cancelled, key removed, no `.part` yet |
| Downloading | body complete, checks pass | Installing | `.part` moved to commit path |
| Downloading | body complete, a check fails | Error | Cache `.part` removed |
| Downloading | network error after retry | Error | Cache `.part` removed |
| Downloading | body larger than 64 MiB | Error | Cache `.part` removed |
| Downloading | cancel | Idle | Cache `.part` removed |
| Installing | `check_install` fails | Error | Commit `.part` removed |
| Installing | commit ok | Installed | `Installed` event with reconciled record |
| Installing | I/O error during commit | Error | Commit `.part` removed; old file kept |
| Installed | (terminal) | — | Key removed from `active` after the event is applied |
| Error | (terminal) | — | Key removed from `active` after the event is applied |
| Idle | (terminal) | — | Key removed from `active` |

### 2.2 Compared with Kotlin

| Kotlin behaviour | Kurobayomi behaviour | Why |
|---|---|---|
| Cancel during download leaves the step unchanged (I:210-214) | Cancel sets `Idle` at once | Kotlin leaves the UI in progress when the DownloadManager row is removed silently (behaviour 7.7) |
| Failed download sets `Error` only through the broadcast | Digest and network failures set `Error` directly | No dependence on a system broadcast |
| Private route skips `Installing` | Every route emits `Installing` | Same UI states for every package |
| Two split rows of one APK → two downloads | One job per package | Avoids two installs of one APK (behaviour 11.1) |

---

## 3. Cancel rules

| Where the job is | Kotlin (`cancelInstall` + queue) | Kurobayomi |
|---|---|---|
| Waiting in DownloadManager, not yet downloaded | DM row removed; no step update; flow stays open (observed hang) | Token cancelled; step `Idle`; `.part` removed |
| Downloading | DM row removed; poll ends silently; no step update | Token cancelled; step `Idle`; `.part` removed |
| Queued in install service | Removed from queue; step `Idle` (IQ `cancelQueue`) | Removed from queue; step `Idle` |
| Active in PackageInstaller | `cancelEntry` returns false: nothing happens (queue may stick) | Not applicable (dropped) |
| Active in Shizuku | `cancelEntry` returns false: nothing happens | Not applicable (dropped) |
| Committing (Rust only) | Not applicable | Ignored; the commit completes and step `Installed` is emitted |

Uninstall while installing (edge case I21): cancel first (same rules), then take the commit lock, then delete the file and emit `Removed`. A commit in progress finishes before the delete starts, because the lock serialises them.

---

## 4. Queue and concurrency

| Resource | Limit | Kotlin | Kurobayomi |
|---|---|---|---|
| Concurrent downloads | none (DownloadManager default) | unlimited | 3 permits (`tokio::sync::Semaphore`) |
| Concurrent commits | one (one active entry) | one (`waitingInstall` CAS) | one (`tokio::sync::Mutex<()>`) |
| Queue order | FIFO | FIFO (`queue.first()`) | FIFO (`mpsc`) |
| Waiting entry during a commit | blocks next commit | blocks | blocks next commit |

The queue is a single consumer task. Requests for a package that already has a job return the existing job's state stream (edge case I19, N5).

---

## 5. Timings and limits

| Name | Value | Where |
|---|---|---|
| Kotlin poll interval | 1 s | `MangaExtensionInstaller.kt:147` |
| Kurobayomi download connect timeout | 10 s | design 9.5, data 6.5 |
| Kurobayomi download total timeout | 120 s | design 9.5, data 6.5 |
| Kurobayomi retry on network error | 1 retry after 2 s | edge case I10 (Proposal) |
| Kurobayomi download size cap | 64 MiB (67,108,864 bytes) | data 6.5 |
| Kurobayomi wasm size cap | 32 MiB | edge case I15 |
| Kurobayomi icon size cap | 256 KiB | data 7.3 |
| Kurobayomi download permits | 3 | design 4 |

---

## 6. Filesystem effects

| Step | Kotlin | Kurobayomi |
|---|---|---|
| Download | DownloadManager file in external files `Download/` | `$XDG_CACHE_HOME/kurobayomi/extensions/download/<pkg>-<versionName>.kuroext.part`, renamed to `.kuroext` on success in the same directory |
| Verify | none | digest, signature, fingerprint (design 6.3) |
| Commit | temp in cache `temp_<id>` (private route), then copy to `filesDir/exts/<pkg>.ext` | copy to `<data>/<pkg>.kuroext.part`, fsync, rename, chmod `0444`, fsync dir |
| Cleanup | `DownloadManager.remove` and `tempFile.delete()` | `.part` deleted on every failure and cancel; startup sweep (T-18a-005) |

Atomic-rename rule: the commit `.part` lives in the target directory, because rename is atomic only within one filesystem (design 14, item 7).

---

## 6.1 Step emission order (for the UI tests)

For a successful install of one package the event sequence is exactly:
1. `Step { key, Pending }`
2. `Step { key, Downloading }` (after permit and first byte)
3. `Step { key, Installing }`
4. `Installed(record)`
5. `Step { key, Installed }`

For a digest mismatch: 1, 2, then `Step { key, Error }`. For a cancel during download: 1, 2, then `Step { key, Idle }`.

Fixture-driven test: `kuro-extensions/tests/install.rs` (T-18a-036 acceptance, edge cases I16 to I19).

---

## 7. Failure modes and expected user-visible result

| Failure | Step shown | Row after | Log |
|---|---|---|---|
| Network down before download | Error | Previous state (installed or available) | `Failed to get extensions from …` is for the index; the download log is `download failed: <reason>` |
| Digest mismatch | Error | Available, not installed | `digest mismatch for <pkg>` |
| Signature invalid | Error | Available, not installed | `signature invalid for <pkg>` |
| Downgrade attempt | Error | Installed version unchanged | `Installed extension version is higher. Downgrading is not allowed.` |
| Signer change | Error | Installed version unchanged | `Installed extension signature is not matched.` |
| Disk full at commit | Error | Installed version unchanged; old file kept | `Failed to copy extension file.` |
| App closed mid-download | none (process gone) | Available | Startup sweep removes `.part` |

The Kotlin log texts for downgrade, signer and copy failure are the ones in `p1-data-and-config.md` section 13. The download and digest log texts are new Rust messages (no Kotlin equivalent).

---

## 8. What is not carried over

- DownloadManager notifications (`VISIBLE_NOTIFY_COMPLETED`): dropped, reason in behaviour section 15.
- The 1 s MIUI ignore window and activity result path: dropped (T-18a-039).
- The foreground service and its queue host: dropped as a service, kept as the queue (T-18a-036).
