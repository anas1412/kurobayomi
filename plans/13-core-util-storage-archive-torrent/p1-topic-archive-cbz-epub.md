# Topic — archives, CBZ, EPUB (local reading and chapter packaging)

Scope: `core/archive/` (all 8 files, see p1-inventory.md). Behaviour details: `p1-behavior.md` section 4. Tasks: T-13a-031 to T-13a-035.

## 1. What the archive module does in Aniyomi

1. **Reads local archives** as manga chapters: a file or folder on the local source is opened and its image entries become pages (`ArchivePageLoader`, `ui/reader/loader/ArchivePageLoader.kt`). Formats are whatever libarchive reads (`readSupportFormatAll`, `readSupportFilterAll`): zip, cbz, rar, cbr, 7z, tar and compressed tar. The complete list is fixed by the AAR build (OQ-13a-6).
2. **Reads EPUB** packages: the page images are taken from the XHTML spine (`EpubReader`), used by the local source and the EPUB loader (`ui/reader/loader/EpubPageLoader.kt`, `source-local/.../metadata/EpubReaderExtensions.kt`).
3. **Writes CBZ** for finished chapter downloads (`ZipWriter`, `MangaDownloader.archiveChapter`): stored (uncompressed) ZIP with the page files as entries.

Everything is read-only except the CBZ writer. No archive is extracted to disk: entries are streamed.

## 2. Format coverage decision

| Option | Coverage | Pros | Cons | Decision |
|---|---|---|---|---|
| A. libarchive via FFI (`libarchive3-sys` or the `libarchive` crate) | Same as Aniyomi (all formats) | exact parity; one code path; streaming with `archive_read_open_memory` | system dependency (`pkg-config libarchive`); unsafe FFI in one module | **Chosen** |
| B. Pure Rust per format (`zip`, `unrar` (C++), `sevenz-rust`, `tar`) | subset | no C library | several crates, `unrar` licence, slower to cover all cases | Rejected |
| C. zip and cbz only | zip, cbz | smallest | drops cbr, 7z, tar: a user loss for some local libraries | Rejected |

Decision (proposal, verified in the spike T-13a-031): option A. The `zip` crate is still used for the CBZ writer (stored entries) and as the zip reader fallback when libarchive is not installed (feature flag `archive-zip-only`, documented as a reduced-format build).

Format fixture matrix (T-13a-031 acceptance): zip (stored and deflate), cbz, cbr (RAR created by `rar a`, test skipped if `rar` missing), 7z, tar, tar.gz, and an encrypted zip (expect a clean error).

## 3. Reader design

- `ArchiveReader::open(path)`: `memmap2::Mmap` read-only. The map lives as long as the reader (Arc). Keeping a mapping open per chapter means a 1 GB archive costs address space only, not RAM (the kernel pages it in on demand).
- `entries()`: opens a new libarchive handle over the map, iterates `archive_read_next_header`, and collects `(name, is_file)`. Names are UTF-8 (`archive_entry_pathname_utf8`); a name that is not valid UTF-8 is skipped.
- `open_entry(name)`: scans headers from the start until the exact name matches (case-sensitive), then returns an `EntryReader` that owns the handle. The scan is O(entries) per page; a 300-page archive costs about 45,000 header reads over a full read (recorded ceiling; index optimisation is a later task, not in this plan).
- `EntryReader: Read` reads through `archive_read_data` into the caller's buffer; drop frees the handle.
- Concurrency: one handle per operation; no shared mutable state; `ArchiveReader` is `Send + Sync`.

### 3.1 The page filter (behaviour 4.3)

1. Keep entries with `is_file == true`.
2. Keep entries where `is_image_name(name, head)` holds: extension in `{avif, gif, heif, jpg, jxl, png, webp}` (case-sensitive, whole name when no dot), or the first 32 bytes sniff to a known image kind.
3. Sort by `natural_cmp_ci` on the name (case-insensitive natural order).
4. Number pages from 0.

Consequences a user can see: `Page01.JPG` is matched only by sniffing (uppercase extension is not in the list); `cover` with no extension is matched by sniffing; `.jpeg` is matched by sniffing. The results are the same as Aniyomi.

### 3.2 Entry names with directories

Archives often store `Chapter 1/001.jpg`. The Kotlin filter keeps the full path as the entry name and the page sort uses the full name. Rust keeps the same behaviour (no directory flattening), so a chapter in a sub-folder sorts as the full path string. Document for the local source screens.

## 4. EPUB design

Algorithm (behaviour 4.5) in five steps: (1) detect the path separator (`META-INF\container.xml` means backslash); (2) read `META-INF/container.xml` and take the first `rootfile[full-path]`; fall back to `OEBPS/content.opf` when absent or empty (deviation D-8); (3) parse the package: `manifest > item` with `media-type="application/xhtml+xml"` keyed by id, then `spine > itemref` by idref in spine order; (4) for each XHTML page resolve the relative path against the package directory and parse its `img[src]` and `image[xlink:href]` (in document order); (5) resolve each image path against the page directory, deduplicate, keep order.

Parsing: `scraper` (html5ever). Reason: jsoup, which the Kotlin uses, parses these XML files with its HTML parser, lower-cases tag names and tolerates broken markup. A strict XML parser would reject packages that Aniyomi reads today. Attribute matching for `xlink:href` uses the namespace prefix and local name, because html5ever stores SVG xlink attributes in the xlink namespace.

Path resolution: pure string normalisation (`resolve_zip_path`, behaviour 4.5 item 7). Symlinks do not exist in zip, so `File.canonicalPath` in the Kotlin is equivalent to lexical normalisation; `..` above the root clamps at the root.

Known Kotlin defects fixed in the port (deviations D-8, D-9, D-10): empty `full-path` falls back; `img` without `src` skipped (the Kotlin adds the directory path); duplicates removed.

## 5. CBZ writer design

- Output file: `<chapter>.cbz_tmp` during writing (`TMP_DIR_SUFFIX` = `_tmp`), renamed to `<chapter>.cbz` on success (`MangaDownloader.kt:639,645`). A crash leaves the `_tmp` file; the cleanup on the next run is the downloader's job (to be verified in the downloader area, not in this one).
- Entries: basenames of the page files (not paths), stored method (no compression: images are already compressed), mtime from the source file (clamped to 1980-01-01 00:00:00 local, since ZIP cannot store earlier times), unix mode from the source (masked to `0o7777`).
- Order: sorted by natural order of names for determinism (Kotlin order is directory listing order; the reader sorts on read, so there is no visible difference).
- ZIP64: not needed for chapters below 4 GiB; the writer errors clearly above that limit (`zip` crate behaviour: enable ZIP64 when an entry exceeds 4 GiB; proposal: enable it, so no error).
- `finish()` flushes and renames. Drop without finish deletes the `_tmp` file (cleanup-on-drop, documented).

## 6. Error handling

| Situation | Result |
|---|---|
| file missing or not an archive | `ArchiveError::Open` (page list empty; the source reports the chapter as broken) |
| truncated archive | `Open` for listing; per-entry read error for the damaged entry; other pages still readable where libarchive can continue |
| entry name not UTF-8 | skipped (listing), reported only in the debug log |
| nameless entry | skipped (deviation D-11) |
| encrypted entry | that entry's read returns an error; the page shows an error state |
| EPUB without package | fallback path; if still missing, `ArchiveError::PackageMissing` |
| CBZ write failure (disk full) | `io::Error`; the `_tmp` file is removed; the download is marked failed |

## 7. Destructive operations and safety

- Archive reading never writes to the archive.
- The CBZ writer creates files under the chapter folder only (path validated by the downloader).
- Filenames of entries come from the source files, which were already sanitised (filename rules in `p1-topic-storage-and-filenames.md`).

## 8. Interactions

- Local source (other area) lists chapters and pages through `ArchiveReader::entries`.
- The reader (other area) uses `open_entry` for each page and `choose_background` on the decoded page (image topic).
- The downloader (other area) uses `CbzWriter` and the image split (image topic).
- Platform: archives are opened by path from `kuro-platform` file choosers; no SAF (T-13a-035).

## 9. Test plan (summary; vectors in p1-edge-cases-and-tests.md sections 9.1 to 9.4)

- Fixture archives per format (section 2 matrix); each returns the listed entries in archive order.
- Page filter and natural sort vector (`1.jpg`, `2.jpg`, `10.jpg`).
- Exact-name lookup (case-sensitive), missing name, handle freed.
- EPUB: path vectors, package fallback, spine filter, `xlink:href`, backslash separator, duplicates.
- CBZ: entry names are basenames, stored method, `zip -T` validation, rename behaviour, `_tmp` cleanup on drop.
- Concurrency: 8 threads listing one archive concurrently.
