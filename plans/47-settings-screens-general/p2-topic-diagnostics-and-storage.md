# Topic: diagnostics pages and storage usage

Scope files: `app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt`, `WorkerInfoScreen.kt`, `BackupSchemaScreen.kt`, and `app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt`. Full behaviour: `p2-behavior.md` sections 7-10. Data: `p2-data-and-config.md` sections 5, 8. Tests: `p2-edge-cases-and-tests.md` sections 3.2, 3.5, 3.6.

## 1. Debug info page map

```
Debug info (PreferenceScaffold)
 |-- Worker info          -> WorkerInfoScreen   (live job dump)
 |-- Backup file schema   -> BackupSchemaScreen (proto text)
 |-- [App info]
 |     Version            "Stable <ver>" | "Debug <sha>" | "Preview r<n> (<sha>)"
 |     Build time         date(app_date_format) + " " + short time
 |     Profile status     DROPPED on Linux
 |     WebView version    -> WebKitGTK version on Linux
 |     (Linux) GTK, Session
 |-- [Device info]
       Model              "<vendor> <product> (<board>)"  (Linux DMI)
       OneUI / MIUI       DROPPED
       Android version    -> OS PRETTY_NAME and kernel on Linux
```

All value rows are read-only. Two rows navigate. Titles that are hard-coded English in Kotlin stay English here.

## 2. Worker info

Data source: the job registry (T-47b-044). Each job is a record `{id, tags, state, next_run, attempts}`.

Three buckets:
- Enqueued: state `ENQUEUED`.
- Running: state `RUNNING`.
- Finished: states `SUCCEEDED`, `FAILED`, `CANCELLED`.

Text per job (exact):

```
Id: <uuid>
Tags:
 - <tag>               (one line per tag, in stored order)
State: <STATE>
Next scheduled run: <date time>     (only when ENQUEUED)
Attempt #<attempts + 1>             (only when ENQUEUED)
<blank line>
```

Empty bucket: `-` followed by newline.

Copy: concatenates the three bucket texts in the order Enqueued, Finished, Running (Kotlin `enqueued + finished + running`), which is also the on-screen order.

Live: the page subscribes to a `watch` receiver while open; on close it drops the receiver (Kotlin `WhileSubscribed`).

Known Kotlin limit: no cap on finished jobs. Rust proposal caps the dump at 500 entries with a final line `... <n> more` (E67, flagged).

## 3. Backup file schema

- Text is the protobuf schema of the `Backup` message. Kotlin generates it at runtime from the serializer descriptor. Rust embeds `proto/backup.proto` with `include_str!` so the displayed schema is the one the decoder uses.
- Field numbers must match the Aniyomi format (area:21). Test T-UT-26 compares them.
- Page: monospace, vertical scroll, selectable, copy action.

## 4. Storage usage

### 4.1 Roots
- Kotlin: the external storage roots that the Android system reports as mounted.
- Linux: mount table entries of real block-device filesystems (list in `p2-data-and-config.md` section 5), plus the data directory's mount, deduplicated by `st_dev`.
- A root without usage data still shows its path.

### 4.2 Numbers
- `available = f_bavail * f_frsize`, `total = f_blocks * f_frsize`.
- Used fraction = `(total - available) / total`, clamped to 0..1; no bar when `total == 0`.
- Caption: `Available: <size> / Total: <size>` using the SI formatter (T-47b-039).

### 4.3 Kotlin defects not carried over
- `total = -1` on stat failure is fed into the progress formula, giving values above 1 (unclamped). Rust hides the bar.
- No refresh after the page opens. Rust refreshes on page show (T-47b-061).

### 4.4 Timeouts
- statvfs runs on the tokio runtime with a 2 s timeout. A hung FUSE mount shows "Storage size unavailable".

## 5. Date formatting

One helper decides the format for all three pages:
- `app_date_format` empty: localised short date and short time.
- Non-empty: the value is a chrono format string (Kotlin: `DateTimeFormatter.ofPattern(format, Locale.getDefault())`).
- Rust default proposal: `%x %X` in the user's locale when available, otherwise `%Y-%m-%d %H:%M`. Document the difference from Java's localised SHORT style.

## 6. Build metadata
- `build.rs` exports `KUROBAYOMI_BUILD_TIME`, `KUROBAYOMI_COMMIT_SHA`, `KUROBAYOMI_COMMIT_COUNT`, and the WebKitGTK version (`pkg-config --modversion webkitgtk-6.0`, fallback `webkit-6.0`).
- Missing build time shows `unknown` (Kotlin shows the raw string on parse failure).
- Reproducible builds: honour `SOURCE_DATE_EPOCH`.

## 7. Tasks
- Debug info: T-47b-040, T-47b-041, T-47b-042, T-47b-048, T-47b-049, T-47b-050, T-47b-051.
- Worker info: T-47b-043, T-47b-044, T-47b-047.
- Schema: T-47b-045, T-47b-047.
- Storage: T-47b-037, T-47b-038, T-47b-039, T-47b-061.

## 8. Drops
- ART profile compilation status (no Android runtime on Linux).
- OneUI and MIUI rows (vendor skins).
- The Android "WebView" version becomes WebKitGTK; the Android System WebView version is not available.
Users lose only the information that is meaningless on Linux (see `p2-behavior.md` section 12).
