# Topic — storage folders, filenames and temporary files (cross-platform rules)

Scope: `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt`, `FileExtensions.kt`, `FFmpegUtils.kt`; `core/common/src/main/java/tachiyomi/core/common/storage/` (`AndroidStorageFolderProvider.kt`, `FolderProvider.kt`, `UniFileExtensions.kt`, `UniFileTempFileManager.kt`). Behaviour: `p1-behavior.md` sections 3.1 to 3.11. Tasks: T-13a-014 to T-13a-020, T-13a-054 to T-13a-056.

## 1. Why this is a topic

Filenames built from manga and episode titles are the one place where this area's behaviour is visible in the user's folders, and they must work on Linux, on NTFS (for the planned Windows port, rule 5 of `plans/addenda/requirements-cross-platform.md`), and on FAT media (SD cards and USB sticks). The Kotlin rules were written for FAT only; the Rust rules extend them.

## 2. The filename pipeline (final form)

Input: a title (manga or episode name, page name, chapter name) and a page number or suffix.

1. Byte budget (for reader and player page names only): `takeBytes(title + chapterName, 250 - byteSize(suffix))`. The 250-byte budget leaves room for the extension and the suffix (`DiskUtil.MAX_FILE_NAME_BYTES`, `DiskUtil.kt:254`).
2. Trim: remove leading and trailing `.` and space characters.
3. Replace forbidden characters with `_`: control characters `0x00..0x1F`, DEL `0x7F`, and `" * / : < > ? \ |`.
4. Empty result becomes `(invalid)`.
5. Reserved names (Windows device names, rule 5): `CON PRN AUX NUL COM1..COM9 LPT1..LPT9` (case-insensitive, with or without an extension, compared on the part before the first `.`) get a leading `_`. Deviation D-1.
6. Component limit: 240 UTF-16 units (Kotlin `take(240)`). If the cut splits a surrogate pair, move the cut back one unit. Deviation D-2.
7. Trim trailing `.` and space again after truncation (the Kotlin does not; the NTFS rule requires it). Deviation, recorded with the reserved-name change.

Test vectors: `p1-edge-cases-and-tests.md` section 6.

## 3. What is not changed

- Existing download folders keep their names: the rules above are the same as the Kotlin for ASCII names, except for the reserved names, which are not produced by normal titles.
- Unicode letters and emoji are kept as they are (no transliteration). This matches Aniyomi and keeps folder names searchable.
- The FAT rule about the 255-character limit is replaced by the 240 UTF-16 units limit (the Kotlin's own comment explains the 15-character margin for FUSE layers; the Rust port keeps 240 for parity).

## 4. Free space and directory sizes

- `free_bytes(path)`: `f_bavail * f_frsize` via `rustix::fs::statvfs`. "bavail" is the space available to an unprivileged user, the same quantity as Android's `availableBlocksLong`.
- `total_bytes(path)`: `f_blocks * f_frsize`.
- Both return `None` when the path does not exist or the call fails (Kotlin returns `-1`).
- `dir_size(path)`: walk with `walkdir` and `follow_links(false)`; sum regular files; ignore directory entries. Symlink loops terminate (deviation D-16).

Consumers: the download pre-check (`MangaDownloader.kt:337`, free space) and the cache size display (`ChapterCache.kt:51`).

## 5. Folders

| Folder | Purpose | Kotlin | Rust (kuro-platform Paths) |
|---|---|---|---|
| Storage root | downloads, backups, covers (chosen by user) | `<external storage>/Aniyomi` | `$XDG_DOWNLOAD_DIR/Kurobayomi` default, user-chosen otherwise |
| Data | TorrServer data, extension copies | app private files | `$XDG_DATA_HOME/kurobayomi/` |
| Cache | temp files, chapter cache | `externalCacheDir` | `$XDG_CACHE_HOME/kurobayomi/` |
| Config | settings file (if area 02 uses one) | shared preferences | `$XDG_CONFIG_HOME/kurobayomi/` |

Rules
- Only kuro-platform reads `XDG_*` (cross-platform rule 1).
- Paths are built with `PathBuf::join`; separators are never typed.
- A user-chosen storage root must be writable: the writability check (T-13a-015) creates and removes a probe file before the first-run step continues.

## 6. Temporary files

- Directory: `<cache>/tmp` (Kotlin `externalCacheDir/tmp`).
- Names: a prefix padded with spaces to at least three bytes (the Kotlin `padEnd(3)` on the UniFile base name, required by `File.createTempFile`), then a random part, then `.tmp`.
- Lifetime: the `tempfile::NamedTempFile` removes the file on drop. `TempFiles::clear()` runs only at startup, before any download starts (race rule in `p1-edge-cases-and-tests.md` section 14).
- The Kotlin `UniFileTempFileManager` has no caller in this repository (dead code). The Rust port creates it only when a feature needs a temporary copy of a source file.

## 7. Copies of installed files (extensions)

- `copy_read_only(source, target, overwrite)`: refuses to overwrite unless asked; copies with a buffer of 8192 bytes; sets the mode to `0o444` after the copy (deviation D-5); a source directory creates the target directory and copies no content (parity, flagged).
- Used by the extension loaders (other area) to protect installed extension files from accidental edits.

## 8. Hash-based cache keys

`disk_key(key) = md5_hex(key as UTF-8)`. The cover and chapter caches use it for file names (`ChapterCache.kt`, `MangaCoverCache.kt`). Keeping the exact function keeps cache file names identical if a cache directory is copied from Android.

## 9. What the user loses (summary)

| Dropped | User-visible effect | Mitigation |
|---|---|---|
| SD card enumeration | cannot pick an SD card from a quick list | folder chooser (portal) |
| MediaScanner indexing | gallery apps are not told about new pages | `.nomedia` still written; desktop indexers not guaranteed to skip it (open question) |
| Storage Access Framework folders | cloud folders such as Google Drive cannot be a download root | copy archives locally; choose a local folder |
| FileProvider share directory | sharing copies the image into a folder first | portal share with the file path |

## 10. Tests (summary)

- Filename vectors (section 2 and `p1-edge-cases-and-tests.md` section 6), including `CON`, a 300-character title, a surrogate boundary and the byte budget.
- Free space on the root filesystem (`Some(n > 0)`), and `None` on a missing path.
- `dir_size` on a tree with symlinks and a loop.
- `create_nomedia` on an existing and a missing directory.
- `copy_read_only` cases (section 7).
- Temp names: prefix padding and suffix; clear twice.
