# Area 13 — core utilities, storage, archives, images, torrent helpers: overview

Area lead overview. Area id: `13-core-util-storage-archive-torrent`. All repo-relative paths are relative to `/home/blackbox/aniyomi-src` (read-only Aniyomi clone). Paths inside this folder are relative to `/home/blackbox/kurobayomi/plans/13-core-util-storage-archive-torrent/`.

## 1. Purpose and scope

Aniyomi's "core/common" and "core/archive" modules hold the low-level code that the rest of the app calls when it touches files, images, archives, preferences and the local TorrServer. This area covers:

- **Preference handles** (`tachiyomi.core.common.preference`): typed wrappers over SharedPreferences, the private and app-state key prefixes that decide what a backup exports, and tri-state filter values.
- **Text helpers**: truncation to UTF-16 units, byte-budget truncation, natural (case-insensitive, numeric-aware) ordering, locale collation, legacy HTML decoding.
- **Storage**: the filename sanitiser that turns manga and episode titles into folder and file names, free space and directory size, `.nomedia`, temporary files, read-only copies of installed extension files, the default storage folder.
- **Archives**: reading CBZ, ZIP, CBR, 7z, tar and EPUB as page sources (libarchive over a memory map), and writing stored CBZ files for chapter downloads.
- **Images**: magic-byte sniffing, decoding, the reader's double-page operations (split, rotate, merge), tall-page splitting at download time, and the automatic page background colour.
- **Torrent helpers**: strict bencode, `.torrent` metadata and info hash, TorrServer HTTP client, the playback routing rule, and the settings validators.
- **Android utilities that are dropped or replaced** (DeviceUtil, GLUtil, DensityExtensions, SAF helpers, FileProvider, RxJava bridge, WebView helpers mapped onto WebKitGTK, toast mapped onto an in-app overlay).

Why it matters for a usable Kurobayomi: almost every P0 path touches this area. Reading a manga means the page list comes from `ArchiveReader` or a folder, each page is sniffed and decoded here, the reader's double-page and background rules run here, and the page file names come from the sanitiser. Opening an anime from a torrent link needs the routing and the TorrServer client. Downloading a chapter needs the free-space check, `.nomedia`, the tall-page split and the CBZ writer. Without this area, no reading path works.

Scope size: 50 files in the inventory (see [p1-inventory.md](p1-inventory.md)); 45 Kotlin source files, 2 Gradle scripts, 2 empty Android manifests, 1 test class. Two test fixture resources are referenced from the test row.

## 2. Module map

### 2.1 Kotlin packages and responsibilities (OBSERVED)

| Kotlin location (repo-relative) | Responsibility | Lines (per inventory) |
|---|---|---|
| `core/archive/src/main/kotlin/mihon/core/archive/` (`ArchiveReader.kt`, `ArchiveInputStream.kt`, `ArchiveEntry.kt`, `EpubReader.kt`, `ZipWriter.kt`, `UniFileExtensions.kt`) | libarchive streaming reader over an mmap, EPUB package walker, stored-ZIP writer, SAF/UniFile bridge | 6 + 63 + 38 + 135 + 73 + 13 |
| `core/common/src/main/java/aniyomi/core/common/torrent/` (`TorrentServerApi.kt`, `TorrentServerUtils.kt`, `TorrentPreferences.kt`, `TorrentHelpers.kt`, `DisabledTorrServerException.kt`) | TorrServer HTTP client, stream URL and tracker push, six torrent preference keys and `ProxyMode`, `.torrent` parser entry point | 96, 30, 51, 95, 3 |
| `core/common/src/main/java/aniyomi/core/common/torrent/bencode/` (`BencodeParser.kt`, `BencodeWriter.kt`, `BencodeValue.kt`) | Strict bencode grammar, canonical encoder, value model with signed-byte key ordering (defect) | 112, 68, 61 |
| `core/common/src/main/java/aniyomi/core/common/torrent/model/` (`Torrent.kt`, `TorrentRequest.kt`) | kotlinx-serializable DTOs for TorrServer JSON | 22, 16 |
| `core/common/src/main/java/eu/kanade/tachiyomi/util/lang/` (`Hash.kt`, `StringExtensions.kt`) | MD5 and SHA-256 hex; chop, truncateCenter, natural compare, byte size, takeBytes, htmlDecode | 44, 67 |
| `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/` (`DiskUtil.kt`, `FileExtensions.kt`, `FFmpegUtils.kt`) | Filename sanitiser, free and total space, directory size, `.nomedia`, media scan, copy-then-read-only, FFmpeg SAF string | 144, 64, 27 |
| `core/common/src/main/java/tachiyomi/core/common/storage/` (`AndroidStorageFolderProvider.kt`, `FolderProvider.kt`, `UniFileExtensions.kt`, `UniFileTempFileManager.kt`) | Default data folder, folder abstraction, name and extension helpers, temp copies (dead code in this snapshot) | 25, 10, 12, 46 |
| `core/common/src/main/java/tachiyomi/core/common/preference/` (`AndroidPreference.kt`, `AndroidPreferenceStore.kt`, `Preference.kt`, `PreferenceStore.kt`, `InMemoryPreferenceStore.kt`, `TriState.kt`, `CheckboxState.kt`) | Typed preference handles, store interface, change flows, in-memory test store, filter states | 194, 79, 76, 43, 97, 16, 47 |
| `core/common/src/main/java/tachiyomi/core/common/util/system/` (`ImageUtil.kt`, `LogcatExtensions.kt`) | Format sniffing, decode and encode, page operations, tall split, background colour, logging helper | 728, 18 |
| `core/common/src/main/java/tachiyomi/core/common/util/lang/` (`CoroutinesExtensions.kt`, `RxCoroutineBridge.kt`, `SortUtil.kt`, `BooleanExtensions.kt`) | Dispatcher helpers, Rx bridge (dropped), locale collator, Boolean to Long | 66, 58, 15, 3 |
| `core/common/src/main/java/eu/kanade/tachiyomi/util/system/` (`ToastExtensions.kt`, `WebViewUtil.kt`, `DeviceUtil.kt`, `GLUtil.kt`, `DensityExtensions.kt`) | Toast, WebView version and user agent, vendor checks (dropped), EGL texture limit (dropped), dp to px (dropped) | 33, 116, 113, 65, 9 |
| `core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt` | String and plural lookup with a backslash-stripping workaround | 30 |
| `core/common/src/main/java/mihon/core/common/extensions/JsonObject.kt` | Empty JSON object constant | 10 |
| `core/archive/build.gradle.kts`, `core/common/build.gradle.kts`, two empty `AndroidManifest.xml` files, `core/common/src/test/.../TorrentHelpersTest.kt` | Dependency declarations, manifests, parser test | 15, 60, 2, 2, 68 |

### 2.2 Rust crate and module map (PROPOSAL)

Everything except the toast lives in `kuro-core`, which must not depend on GTK. The toast lives in the GUI crate. WebView glue lives in `kuro-platform` behind the `WebViewHost` trait. Full signatures are in [p1-rust-design.md](p1-rust-design.md) section 3.

```
crates/kuro-core/src/
  util/     text.rs natural.rs collate.rs hash.rs json.rs lang.rs
  prefs/    mod.rs (Pref<T>, RawStore, EnumPref, keys) memory.rs filter.rs
  storage/  mod.rs (FolderProvider) filename.rs space.rs dirs.rs nomedia.rs temp.rs copy.rs names.rs
  archive/  mod.rs (ArchiveReader) libarchive.rs (private FFI) epub.rs zip_writer.rs
  image/    mod.rs sniff.rs decode.rs ops.rs background.rs props.rs
  torrent/  bencode.rs meta.rs model.rs client.rs supervisor.rs prefs.rs route.rs
  web.rs    (UA transform, Chrome major parse)
  error.rs  (UtilError umbrella, thiserror)
  log.rs    (priority to tracing level)
  rt.rs     (spawn_io, spawn_net, on_ui, JoinSet for non-cancellable work)
crates/kuro-core/tests/   torrent_meta.rs, plus the vectors in p1-edge-cases-and-tests.md
crates/kuro-core/tests/fixtures/torrent/   two .torrent fixtures copied from core/common/src/test/resources
src/toast.rs              in-app overlay toast (GUI crate, T-13a-053)
```

Crate choices (full table in [p1-rust-design.md](p1-rust-design.md) section 5): `tokio`, `reqwest`, `serde`/`serde_json`, `thiserror`, `tracing`, `zip` (write Stored only; read deflate), libarchive via `libarchive3-sys` or the `libarchive` crate (decided by spike T-13a-031), `memmap2`, `scraper`, `image` 0.25, `jxl-oxide`, `libheif-rs` (optional `heif` feature), `icu_collator`, `md-5`, `sha2`, `sha1`, `tempfile`, `walkdir`, `rustix` (statvfs), `form_urlencoded`, `url`. Rejected: `natord`, `unrar`, `hex`, bencode crates, `rust_icu`.

## 3. Reading guide

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Every in-scope file with LOC, every declaration, the port decision (Port, Rewrite, Replace-with-crate, Replace, Drop, Test) and the task ids. |
| [p1-behavior.md](p1-behavior.md) | The algorithms and rules as the Kotlin does them (OBSERVED), with the Rust proposal (PROPOSAL) for each: preferences, text, storage, archives, image sniffing and operations, background algorithm, bencode, TorrServer, Android utilities. |
| [p1-data-and-config.md](p1-data-and-config.md) | Preference keys with types and defaults, enum values, data model fields, folders on disk, TorrServer endpoints, the exact default tracker list, message keys, constants, the Linux `Paths` mapping and the two torrent fixtures' expected values. |
| [p1-rust-design.md](p1-rust-design.md) | Crate and module layout, error enums, trait and function signatures, threading model, crate choices with reasons, the Android-to-Linux mapping table, pitfalls, mockup plug-in points, open design questions. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Upstream test to port, every test vector (natural order, text, filenames, preferences, storage, archives, EPUB paths, sniffing, split plans, background, bencode, TorrServer client, supervisor, validators), fixtures, failure modes, races, deviations D-1 to D-17, open questions OQ-13a-1 to OQ-13a-6. |
| [p1-tasks.md](p1-tasks.md) | The 58 implementation tasks T-13a-001 to T-13a-058 in repository task format (lint passes). |
| [p1-topic-archive-cbz-epub.md](p1-topic-archive-cbz-epub.md) | Deep dive: format coverage decision for libarchive, reader design, page filter, EPUB algorithm and its Kotlin defects, CBZ writer, error table. |
| [p1-topic-image-processing.md](p1-topic-image-processing.md) | Deep dive: the image pipeline (sniff, dimensions, decode, operations, background), codec matrix, colour management, memory and threading, the pixel rules table. |
| [p1-topic-storage-and-filenames.md](p1-topic-storage-and-filenames.md) | Deep dive: the filename pipeline in final form (with Windows reserved names), what is not changed, free space and directory size, folder table, temp files, read-only copies, what the user loses. |
| [p1-topic-torrserver-lifecycle.md](p1-topic-torrserver-lifecycle.md) | Deep dive: why the Android AAR cannot be reused, the child-process supervisor design and state machine, tracker delivery options, proxy, playback routing, timings, risks. |
| [p1-ui-parity.md](p1-ui-parity.md) | The user-visible behaviour this area drives: toast, torrent settings rows and their enabled rules, legal notice, torrent status row, reader background, folder step, mockup gaps. |

## 4. Key design decisions and open questions

Each item states what the Kotlin does (OBSERVED), what is proposed (PROPOSAL), and the recommendation for the user's call.

### 4.1 Decisions already proposed in the plan

| # | Topic | OBSERVED | PROPOSAL | Recommendation to the user |
|---|---|---|---|---|
| D-A | Archive format coverage | libarchive with `readSupportFilterAll` and `readSupportFormatAll` (`core/archive/src/main/kotlin/mihon/core/archive/ArchiveInputStream.kt`) | libarchive via FFI for all formats; `zip` crate for writing and as a reduced-format fallback feature `archive-zip-only` | Accept libarchive as a system dependency. Arch-based systems already have it because pacman links it (verify with `pacman -Qi libarchive` on the machine before the spike). |
| D-B | TorrServer runtime | In-process gomobile AAR `io.github.secozzi:torrserver:0.1.1` (`gradle/aniyomi.versions.toml:9,20`) | Supervised child process with HTTP on `127.0.0.1` (see [p1-topic-torrserver-lifecycle.md](p1-topic-torrserver-lifecycle.md)) | Accept, but the user must be able to install a TorrServer binary. The app never downloads it. Torrent playback is P2, so the rest of the app ships without it. |
| D-C | Bencode key order | Signed-byte comparison, so a key starting with `0x80` or higher sorts before `a` and the dictionary is rejected (`core/common/src/main/java/aniyomi/core/common/torrent/bencode/BencodeValue.kt`) | Unsigned byte order per BEP 3 (deviation D-7) | Accept. It only accepts more valid torrents; the info hash is unchanged for canonical input. |
| D-D | Windows reserved names | Not handled in `DiskUtil.buildValidFilename` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt`) | `CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9` get a leading `_` (D-1) | Accept. It changes nothing for normal titles and keeps the NTFS path safe for a later Windows port (`plans/addenda/requirements-cross-platform.md` rule 5). |
| D-E | Filename component limit | `take(240)` in UTF-16 units; a cut can split a surrogate pair (D-2) | Move the cut back one unit when it splits a pair | Accept. |
| D-F | Hardware bitmap, EGL texture limit, `pref_hardware_bitmap_threshold` | Android tile size and GPU limits (`GLUtil.kt`, `ImageUtil.kt`) | Dropped; the key is ignored and not migrated (T-13a-030) | Accept. GTK manages textures. No user-visible loss on the desktop. |
| D-G | Toast | Android system toast, about 2 s and 3.5 s (`ToastExtensions.kt`) | In-app overlay in `src/toast.rs`, 2 s and 3.5 s, one at a time, never takes focus | Accept. |
| D-H | Storage root | `<external storage>/Aniyomi` (`AndroidStorageFolderProvider.kt`) | `$XDG_DOWNLOAD_DIR/Kurobayomi`, user can pick another folder; existing Aniyomi folders are read as they are | Accept. The user keeps any existing Aniyomi downloads. |
| D-I | Tall-page split height | `2 * max(display width, height)` of the device (`ImageUtil.kt`, `getDisplayMaxHeightInPx`) | Monitor geometry at startup from GDK, times 2; fallback constant `2 * 1080` for tests with no display | Accept. Note the value depends on the monitor, not the window (same as Android). |
| D-J | Background landscape test | Device orientation | Viewport wider than tall when the page is set | Accept. |
| D-K | Proxy mode `None` | Sent as `"tracker"` (defect, OQ-13a-3) | Send no proxy mode field when None | Accept after the stub-server test. |
| D-L | Natural order | Java `long` wrap and unsigned compare for digit runs over 19 digits | Exact port, `wrapping_mul`, `u64` compare | Accept. |
| D-M | Archive page lookup | Linear header scan per page (`ArchiveReader.getInputStream`) | Same, plus a P2 follow-up index (name to header offset) if profiling demands it | Accept now, measure in T-13a-031. See section 7 for the solid-archive cost. |

### 4.2 Open questions (block matching tasks)

| ID | Question | Recommendation | Owner task |
|---|---|---|---|
| OQ-13a-1 | How does the Linux TorrServer receive the tracker list? The AAR's `addTrackers` has no documented HTTP equivalent in this repository. | Spike the chosen binary for a settings endpoint first. If one exists, push the list after each start. If not, append `&tr=<url-encoded>` to each magnet at add time. The fallback works with any build and should ship either way. | T-13a-041 |
| OQ-13a-2 | Index `0` for multi-file torrents. `FileStats.id` is 1-based, but uploads and magnets without `index=` request index `0` (`PlayerActivity.kt:1160,1178`). | Test a two-file torrent on the chosen build. Make file 1 play for both `index=0` and `index=1`, and record the rule in the T-13a-044 commit. | T-13a-044 |
| OQ-13a-3 | Proxy mode `None` maps to `"tracker"` on the Kotlin side, so the server gets a proxy mode with no proxy. | Send no proxy mode field for `None`. Confirm against the binary that no proxy is used. | T-13a-043 |
| OQ-13a-4 | Keep the Android-spoof transform of the WebView user agent (`WebViewUtil.kt`, regex `; Android .*?\)`)? | Keep the transform for parity, and use the same string in WebKitGTK and in `reqwest`, because a Cloudflare clearance is tied to the user agent. The decision belongs to area:12-network-stack. | T-13a-052 |
| OQ-13a-5 | Are HEIF and AVIF needed at all? They need a system `libheif` and an AV1 decoder choice. | Make the `heif` feature off by default. Keep sniffing for both kinds so the name check still works. A page that needs them shows "feature disabled" in the page error state. Turn the feature on if the user reads such files. | T-13a-022 |
| OQ-13a-6 | Libarchive binding: `libarchive3-sys` or the `libarchive` crate? | Use `libarchive3-sys` behind a small safe wrapper, unless the spike shows the crate covers `archive_read_open_memory` and the format set. The FFI is the only `unsafe` in the area and must be one module. | T-13a-031 |

### 4.3 Questions only the user can answer

1. Is it acceptable that torrent playback needs a separately installed TorrServer binary on Linux? Recommendation: yes. Torrent playback is P2 and the rest does not depend on it.
2. Should HEIF and AVIF pages work out of the box? Recommendation: no for the first release (OQ-13a-5).
3. Should `.nomedia` keep being written? Recommendation: yes, so folders copied from Android stay recognised. Whether desktop indexers honour `.nomedia` is not verified and the docs do not claim it.
4. Is a stored CBZ with ZIP64 for entries above 4 GiB wanted? Recommendation: yes (proposal in [p1-topic-archive-cbz-epub.md](p1-topic-archive-cbz-epub.md) section 5).

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Needs | From | What exactly | Blocks |
|---|---|---|---|
| Cargo workspace, `kuro-core` and `kuro-platform` crates, `Paths` (`data()`, `cache()`, `config()`, `downloads_default()`), app context, startup order (`TempFiles::clear` before downloads), `JoinSet` drain on shutdown | area:01-foundation-app-bootstrap (task `T-01a-001` and its successors) | Crates, paths, lifecycle | T-13a-001, T-13a-015, T-13a-041, T-13a-048 |
| A settings store that implements `RawStore` (read, write, delete, broadcast of changed keys), the settings table, the settings screen, and the backup filter that excludes `__PRIVATE_` and `__APP_STATE_` keys | area:02-preferences-system | `RawStore` trait; `PreferenceBackupCreator` filter | T-13a-002, T-13a-005, T-13a-042 |
| Shared HTTP client (`reqwest` in Rust, `NetworkHelper` in Kotlin), `awaitSuccess`-equivalent error mapping, the Cloudflare and WebView flow, the user-agent setting | area:12-network-stack | `reqwest::Client`; WebKitGTK flow; UA setting | T-13a-040, T-13a-052 |
| Platform traits: `Opener` (portal open and share), `Notifier` (torrent status failures), `WebViewHost` (WebKitGTK). Their owning area is not named in this folder's docs; expected to be 53-platform-manifest-intents-workers. Confirm with that area. | kuro-platform (owner to confirm) | Trait signatures | T-13a-040, T-13a-052, T-13a-055 |
| Build packaging: libarchive system dependency, optional `libheif`, and the packaged TorrServer binary path (`<data>/bin/torrserver`) | 54-build-release-distribution-docs | Packaging decisions | T-13a-022, T-13a-041 |
| The reader's and downloader's call sites (the consumers listed below) | 43-reader-core, 44-reader-viewers, 23-downloads-manga, 24-downloads-anime, 25-downloads-shared | Call-site wiring, page widget | T-13a-024 to T-13a-029, T-13a-028, T-13a-034 |

### 5.2 What others need from this area

| Consumer area | Functions used (repo-relative caller in the Kotlin) | Tasks |
|---|---|---|
| 14-source-local-and-metadata | `ArchiveReader` listing and `open_entry`, `EpubReader`, page filter and natural sort (`LocalMangaSource.kt`, `EpubReaderExtensions.kt`) | T-13a-031, T-13a-032, T-13a-033 |
| 43-reader-core, 44-reader-viewers | `is_image_name` and sniff, `decode_rgba`, `dimensions`, `is_animated_supported`, `choose_background`, `split_in_half`, `rotate_quarter`, `split_and_merge`, `take_bytes` for page names (`PagerPageHolder.kt`, `WebtoonPageHolder.kt`, `ReaderViewModel.kt`) | T-13a-021 to T-13a-029, T-13a-011 |
| 23-downloads-manga, 24-downloads-anime, 25-downloads-shared | `sanitize_component`, `free_bytes`, `create_nomedia`, `split_tall`, `CbzWriter`, `TempFiles`, `extension_for` (`MangaDownloader.kt`, `AnimeDownloader.kt`) | T-13a-014, T-13a-016, T-13a-018, T-13a-019, T-13a-028, T-13a-034 |
| 45-player-core, 46-player-controls-ui | Torrent routing and play link, TorrServer supervisor, toast, `take_bytes` for player page names (`PlayerActivity.kt`, `PlayerViewModel.kt`) | T-13a-040 to T-13a-044, T-13a-053, T-13a-011 |
| 15-source-api-manga, 16-source-api-anime | `get_torrent_info` and the `ServerUnavailable` and `ServerTimeout` mapping (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/TorrentUtils.kt`) | T-13a-038, T-13a-040 |
| 18-extensions-manga, 19-extensions-anime | `copy_read_only`, `sha256_hex` for extension signatures (`MangaExtensionLoader.kt:420`, `AnimeExtensionLoader.kt`) | T-13a-013, T-13a-020 |
| 34-images-covers-cache | `disk_key` (MD5 of the key), `dir_size`, `sniff` (`ChapterCache.kt`, `MangaCoverCache.kt`) | T-13a-013, T-13a-016, T-13a-021 |
| 42-ui-updates-history-stats-storage-upcoming | `free_bytes`, `total_bytes`, `dir_size` for the storage screen | T-13a-016 |
| 07-domain-entries-items, 37-ui-library | `Collator::compare` (ICU primary strength) for chapter and library sorting (`ChapterSorter.kt`), and `natural_cmp_ci` for natural order | T-13a-009, T-13a-010 |
| 37-ui-library, 40-ui-browse-manga, 41-ui-browse-anime | `CheckboxState` and `TriState` filter states | T-13a-006 |
| 28-tracker-anilist, 32-tracker-mangaupdates-simkl | `html_decode_legacy` for tracker titles (`ALAnime.kt`, `ALManga.kt`, `MURecord.kt`) | T-13a-012 |
| 21-backup-format-and-create, 22-backup-restore | Key prefixes `__PRIVATE_` and `__APP_STATE_` and the backup filter | T-13a-005 |
| 47-settings-screens-general, 48-settings-player-and-scripts | Torrent rows, validators (`validate_port`, `validate_proxy_url`), the legal notice, the reset rule | T-13a-042, T-13a-043 |
| 51-i18n | Format strings and plural rules without the backslash-stripping workaround | T-13a-050 |
| 36-ui-shell-navigation-security-webview | `WebViewHost` default settings and `get_html` | T-13a-052 |
| All areas | `tracing` priority mapping and the error chain (`LogcatExtensions.kt`); dispatch helpers `spawn_io`, `spawn_net`, `on_ui` | T-13a-048, T-13a-049 |

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, read from `/home/blackbox/kurobayomi/plans/TASKS.csv` rows whose file starts with `13-core-util-storage-archive-torrent/`.

**Counts: 58 tasks.** By priority: P0 19, P1 22, P2 9, P3 8. By effort: S 43, M 13, L 2, XL 0.

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 14 | 4 | 1 | 0 | 19 |
| P1 | 16 | 6 | 0 | 0 | 22 |
| P2 | 5 | 3 | 1 | 0 | 9 |
| P3 | 8 | 0 | 0 | 0 | 8 |
| Total | 43 | 13 | 2 | 0 | 58 |

P0 tasks by id: S = T-13a-001, 003, 004, 008, 009, 010, 013, 015, 016, 023, 024, 032, 048, 049; M = T-13a-002, 014, 021, 022; L = T-13a-031.

Effort ceilings using the lead's definitions (S ≤ 1 day, M ≤ 3 days, L ≤ 1.5 weeks): P0 at most about 34 days, P1 about 34 days, P2 about 22 days, P3 about 8 days. These are ceilings, not estimates.

### 6.1 P0 critical path, in order

1. `T-01a-001` (area:01-foundation-app-bootstrap). Blocks everything in this area.
2. `T-13a-001` (S): crate layout, features, and the dependency mapping table.
3. In parallel after step 2: `T-13a-009` (S, natural order), `T-13a-013` (S, hashes), `T-13a-049` (S, logging), `T-13a-008` (S, chop), `T-13a-010` (S, collator), `T-13a-016` (S, free space), `T-13a-015` (S, folder, also needs `T-01a-001`), `T-13a-048` (S, dispatch, needs `T-01a-001`).
4. `T-13a-021` (M, sniffing), which needs step 2. Then `T-13a-023` (S, animated rules) and `T-13a-022` (M, decode), which need `T-13a-021`. Then `T-13a-024` (S, wide detection).
5. `T-13a-031` (L, libarchive listing), which needs `T-13a-009` and `T-13a-021`. Then `T-13a-032` (S, exact-name entry stream). This is the longest serial chain: `T-13a-001` (S), `T-13a-021` (M), `T-13a-031` (L), `T-13a-032` (S).
6. Preferences, in parallel: `T-13a-002` (M, needs `area:02-preferences-system`), then `T-13a-003` and `T-13a-004` (S). This is the second blocking external dependency.
7. `T-13a-014` (M, filename sanitiser). Needed by P1 downloads (`T-13a-028`, `T-13a-034`) and by reader page names.

Parallel external blockers: area:01 (crates, paths, dispatch) and area:02 (RawStore). Everything else in P0 can start on day one with fixtures.

### 6.2 Risks

1. **TorrServer binary and API (OQ-13a-1, OQ-13a-2, OQ-13a-3).** The Linux behaviour depends on a binary that is not in this repository. Its tracker delivery and multi-file index rules are unverified. Mitigation: spike first in T-13a-041 and T-13a-044; ship torrent as P2 so it does not block the reader.
2. **libarchive FFI (OQ-13a-6).** It is the only `unsafe` code in the area and a system dependency. Mitigation: one module, the spike decides between `libarchive3-sys` and the `libarchive` crate; fixtures per format (zip, cbz, cbr, 7z, tar, tar.gz, encrypted zip).
3. **Solid RAR and 7z cost (new, not in the Kotlin docs).** `open_entry` scans headers from the start for each page. For solid archives, reading an entry also means decompressing the entries before it, so a full chapter read costs roughly pages times archive size in CPU. The Kotlin has the same behaviour (OBSERVED through libarchive). Mitigation: benchmark a 300-page solid RAR in T-13a-031; if it is slow, keep the last decoded entry or extract the chapter once to the cache. The second option is not planned in any task yet.
4. **Background reference data (T-13a-029).** `background_reference.json` is to be generated by a Kotlin-port harness. The repository has no JVM harness for it, so the reference has to come from a line-by-line port (the planning work used Python). Risk: the reference matches the port, not the Kotlin. Mitigation: record the port's version and run the Kotlin logic once on the synthetic images if a JVM is available.
5. **Codec differences.** JPEG q100 output differs by a few bytes from Skia (noted in [p1-rust-design.md](p1-rust-design.md) section 6). CMYK JPEG is not decodable by the `image` crate in all versions. Mitigation: fixture and an explicit "unsupported CMYK JPEG" message.
6. **ICU versus Java collation.** Primary-strength collation is equal for the vectors but tailoring may differ for some locales (German `ß`). Mitigation: vectors in section 4 of [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md); differences are recorded, not failures.
7. **Natural order equivalence.** The port must be exact. Mitigation: proptest against the reference port (T-13a-009).
8. **Torrent path security.** A local server on `127.0.0.1` with no authentication. Mitigation: bind check (no `0.0.0.0` listener) in the supervisor test.
9. **Documentation line references.** Several citations are wrong (see section 8). An implementer who trusts the line numbers will look at the wrong code. Mitigation: grep by function name; corrections below.

## 7. Definition of done for this area

Observable behaviours in Kurobayomi. Each one is checkable by a person or by a test named in the task list.

1. **Local folder and archive reading.** Opening a folder of images, or a `.cbz`, `.zip`, `.cbr`, `.cb7`, `.tar` or `.epub`, lists the pages in natural case-insensitive order (`1.jpg`, `2.jpg`, `10.jpg`). A page named `P01.JPG` is listed only if its bytes sniff as an image. (T-13a-031, T-13a-033, T-13a-009.)
2. **Page decode and display.** Every fixture format in the matrix shows its first page. A truncated page shows the page error state; the app does not crash. (T-13a-022.)
3. **Double-page reading.** A page wider than tall is detected from headers only. With split on, the left and right halves show in the right order for the reader's direction; with rotate-to-fit on, the page turns a quarter turn. (T-13a-024, T-13a-025, T-13a-026.)
4. **Webtoon merge.** A wide page in webtoon mode with split on becomes a `W/2 x 2H` page with the upper half chosen by the invert rule. (T-13a-027.)
5. **Automatic background.** A white page gets a solid white background, a page with dark borders gets a black or two-band background, and a page 100 to 109 px wide shows no error page. (T-13a-029.)
6. **Animated images.** A GIF page plays as an animation and never gets a computed background. (T-13a-023.)
7. **Filenames.** A manga titled `CON` downloads to `_CON`; a title with `:` or `?` gets `_` in those positions; a 300-character title is cut to 240 UTF-16 units; the reader's page file name for "Naruto" chapter "Ch 1" page 7 is `Naruto - Ch 1 - 7`. (T-13a-014.)
8. **Chapter download packaging.** A finished chapter is `<chapter>.cbz`, opens with `zip -T` and in the reader, and has no `_tmp` file left after success. A failed write leaves no `_tmp` file. (T-13a-034.)
9. **Tall pages.** With "Split tall images" on, a page at least four times as tall as wide downloads as parts `NNN__001.jpg` through `NNN__003.jpg` for a 10000 px page with a 4320 px optimal height (the worked example in [p1-behavior.md](p1-behavior.md) section 5.10), with no `NNN.jpg`. A corrupted tall page keeps the original and the chapter still completes. (T-13a-028.)
10. **Free space.** Starting a download when the destination has less free space than needed shows the shortfall message; a missing folder reports no space rather than crashing. (T-13a-016.)
11. **Folder step.** First-run storage shows the default `Kurobayomi` folder; choosing a read-only folder shows "Selected folder is not writable"; no code outside `kuro-platform` reads `XDG_*`. (T-13a-015.)
12. **Preferences.** A stored wrong-typed primitive is reset to its default; an unknown enum name keeps the stored value and returns the default; the default tracker list is exactly the 26 lines in [p1-data-and-config.md](p1-data-and-config.md) section 6. (T-13a-002, T-13a-004, T-13a-042.)
13. **Backup keys.** A backup export never contains a `__PRIVATE_` or `__APP_STATE_` key. (T-13a-005.)
14. **Torrent settings.** Turning TorrServer on shows the legal notice once. Port `0` shows "Running on port N" after start; turning it off shows "Stopped". Port `65536` is refused with `pref_player_torrents_port_error`, and proxy `ftp://h` with `pref_player_torrents_proxy_url_invalid_protocol` (keys in [p1-data-and-config.md](p1-data-and-config.md) section 7; behaviour in [p1-ui-parity.md](p1-ui-parity.md) section 2). (T-13a-041, T-13a-042, T-13a-043.)
15. **Torrent playback.** With torrents on and a binary installed, a magnet link plays through TorrServer. With no binary, the status row says "Failed to start: TorrServer binary not found" and playback shows the toast "TorrServer is not running". (T-13a-041, T-13a-044.)
16. **Toast.** A toast shows for about 2 s (short) or 3.5 s (long), replaces the visible one on a new message, closes on click or Escape, and never takes keyboard focus from the entry being typed in. An empty message shows nothing. (T-13a-053.)
17. **Tests.** `cargo test -p kuro-core` passes every vector in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) sections 2 to 11 and the two torrent fixture tests in section 1.
18. **Dependency hygiene.** `cargo tree -p kuro-core --no-default-features` shows no `gtk` crate. No Rust code in this area depends on an Rx crate, EGL, or a `content://` URI.

## 8. Errata found while spot-checking the source

Corrections for the part documents. The part documents have not been edited for these; use the corrected references.

1. **`core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt`** is 67 lines. The line references in [p1-behavior.md](p1-behavior.md) section 2 (`:56-62`, `:68-76`, `:81-84`, `:89-104`, `:109-111`) and in the caller map are wrong. Actual start lines: `chop` 12, `truncateCenter` 24, `compareToCaseInsensitiveNaturalOrder` 37, `byteSize` 45, `takeBytes` 53, `htmlDecode` 65.
2. **`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt`** is 144 lines. The references in [p1-behavior.md](p1-behavior.md) section 3 (`:220-236`, `:241-249`, `:253-254`, `:162-193`, `:147-157`, `:195-213`, `:129-141`, `:143-145`), in [p1-data-and-config.md](p1-data-and-config.md) section 10 (`:251`, `:254`), and in [p1-topic-storage-and-filenames.md](p1-topic-storage-and-filenames.md) (`:254`, `:235`) are out of range. Actual: `getExternalStorages` 18, `hashKeyForDisk` 32, `getDirectorySize` 36, `getTotalStorageSpace` 51, `getAvailableStorageSpace(File)` 63, `getAvailableStorageSpace(UniFile)` 75, `createNoMediaFile` 87, `scanMedia` 100, `buildValidFilename` 109, `take(240)` at 124, `isValidFatFilenameChar` 130, `NOMEDIA_FILE` 140, `MAX_FILE_NAME_BYTES` 143.
3. **`core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt`** (728 lines): the function starts are correct for `isImage` (39), `isAnimatedAndSupported` (72), `splitInHalf` (129), `rotateImage` (150), `splitAndMerge` (168), `chooseBackground` (338), `optimalImageHeight` (609), `getDisplayMaxHeightInPx` (727). `isTallImage` is at 206 (the doc cites 208 for the ratio test, which is the same function). `splitTallImage` starts at 214, not 201.
4. **`gradle/aniyomi.versions.toml`**: the `torrserver` version is at lines 9 and 20. [p1-topic-torrserver-lifecycle.md](p1-topic-torrserver-lifecycle.md) section 1 cites `:9,19`; the correct line is 20.
5. **Background algorithm edge case** (checked by hand, correct as written in [p1-behavior.md](p1-behavior.md) section 5.12): for widths 100 to 109, `left = 2` and `offsetX = 1`, so the offset column for `rightOffsetX` is `W`, outside the image. The rule holds for 100 to 109 only; `W = 110` gives `left = 3`, no overflow.
6. **Natural order reference value** (checked): `x9999999999999999999999` as a wrapped unsigned 64-bit value is `1864712049423024127`, which is greater than 1, so the sign in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 2 (`1`) is right.
7. **Tracker list** (checked): the 26 lines are 967 characters including the 25 newline separators, which matches [p1-data-and-config.md](p1-data-and-config.md) section 6.
8. **Short area tokens in prose** were normalised to full ids in [p1-rust-design.md](p1-rust-design.md) (the `WebView` row now reads `area:12-network-stack`). Task `Depends` tokens already use full ids.

## 9. Lint and export status

- `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/13-core-util-storage-archive-torrent/p1-tasks.md` prints `ok`. It is the only `*tasks*.md` file in this folder.
- Task ids were not changed.
- `tasks.py export` reports 3607 tasks with 4 errors, all outside this area: `40-ui-browse-manga/p1-tasks.md` lines 37, 83, 158, 537 cite the unknown area `area:41-ui-browse-manga-part-2`. Owner: area 40.
