# Area 13 — Implementation tasks (core utilities, storage, archives, images, torrent helpers)

Format is the repository task format (lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint <file>`). Priorities: P0 needed so Kurobayomi is usable daily (library, reader, local archives, preferences, storage paths); P1 parity features most users rely on; P2 long-tail (torrent playback); P3 Android-only with a recorded drop or replacement. Design: `p1-rust-design.md`. Behaviour: `p1-behavior.md`. Data: `p1-data-and-config.md`. Tests and vectors: `p1-edge-cases-and-tests.md`. UI: `p1-ui-parity.md`. Every task names its sources in the repository.

### T-13a-001 — kuro-core module layout and Cargo features for this area
- Priority: P0
- Effort: S
- Depends: T-01a-001
- Sources: core/common/build.gradle.kts, core/archive/build.gradle.kts, core/common/src/main/AndroidManifest.xml, core/archive/src/main/AndroidManifest.xml
- Acceptance:
  - `crates/kuro-core/src/{util,prefs,storage,archive,image,torrent}/mod.rs` exist and compile with `cargo check -p kuro-core`
  - the Cargo features `archive`, `heif`, `torrent` exist; `cargo tree -p kuro-core --no-default-features | grep -c gtk` prints 0
  - the mapping of each Gradle dependency (jsoup, libarchive, unifile, logcat, rxjava, okhttp, okio, coil image decoder, natural comparator, quickjs, ffmpeg-kit, TorrServer) to a Cargo dependency or a drop is written as a table in `crates/kuro-core/src/lib.rs` doc comment

### T-13a-002 — Typed preference handles over the settings store
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system, T-13a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt, core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt, core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt
- Acceptance:
  - `Pref<T>` provides `get`, `set`, `is_set`, `delete`, `default_value`, `watch` with the semantics of behaviour 1.2
  - a primitive key holding a value of the wrong kind is deleted on `get` and the default is returned (test: write string `abc` under an `i64` key, get returns default and `is_set` is false)
  - an object key holding an undecodable value returns the default and keeps the value (test)
  - `watch()` starts with the current value and conflates 1000 rapid writes to the last value (test)

### T-13a-003 — In-memory preference store for tests and previews
- Priority: P0
- Effort: S
- Depends: T-13a-002
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/InMemoryPreferenceStore.kt
- Acceptance:
  - `InMemoryStore` writes through to its map: `set` then a new `get` returns the new value (deviation D-4)
  - `get_string_set` is implemented and returns the stored set (the Kotlin throws `NotImplementedError`; deviation D-3)
  - used by all `kuro-core` unit tests through `kuro-testkit` (grep: no test constructs a real settings store)

### T-13a-004 — Enum-valued preferences (getEnum semantics)
- Priority: P0
- Effort: S
- Depends: T-13a-002
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt
- Acceptance:
  - `EnumPref<E>` stores the constant name and returns the default for an unknown name without deleting the stored value
  - test: `ProxyMode` stored as `Peers` reads back `Peers`; stored `Nope` reads back the default and `is_set` stays true

### T-13a-005 — Private and app-state key prefixes and the backup filter predicate
- Priority: P1
- Effort: S
- Depends: T-13a-002, area:02-preferences-system
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt
- Acceptance:
  - `keys::is_private`, `private_key`, `is_app_state`, `app_state_key` produce the literal prefixes `__PRIVATE_` and `__APP_STATE_`
  - backup export (area 02 code) excludes both prefixes; a test exports a store containing one key of each kind and asserts neither appears

### T-13a-006 — Tri-state filter cycle and checkbox states
- Priority: P1
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt, core/common/src/main/java/tachiyomi/core/common/preference/CheckboxState.kt
- Acceptance:
  - `TriState::next` cycles Disabled, EnabledIs, EnabledNot, Disabled
  - `TriCheck::next` cycles Unset, Include, Exclude, Unset; `CheckState::next` alternates Checked and Unchecked
  - `as_check_state` and `map_as_check_state` match `asCheckboxState` and `mapAsCheckboxState` on a three-item test list

### T-13a-007 — Preference helper operations
- Priority: P1
- Effort: S
- Depends: T-13a-002
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt
- Acceptance:
  - `get_and_set`, `delete_and_get`, `Pref<BTreeSet>::insert` and `remove`, `Pref<bool>::toggle` exist and match the behaviour in 1.4
  - test: toggle from false returns true and persists; insert then remove leaves the set unchanged

### T-13a-008 — Text truncation helpers (chop and truncate_center)
- Priority: P0
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - `chop("abcdef", 5, "…")` returns `abcd…` and counts UTF-16 units
  - `truncate_center("abcdefghij", 7, "...")` returns `ab...ij`
  - a replacement longer than the limit returns `TextError::ReplacementTooLong` (no panic)

### T-13a-009 — Natural order comparator (case-insensitive, UTF-16 units)
- Priority: P0
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - all vectors in p1-edge-cases-and-tests.md section 2 return the listed sign (`file2 < file10`, `a < B`, `01a < 1b`, `img001 == img1`)
  - a proptest of 10,000 random short ASCII pairs agrees with the reference port in section 2.1
  - the `x9999999999999999999999` vs `x1` case wraps like the Java `long` arithmetic

### T-13a-010 — Locale-aware primary collation
- Priority: P0
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt, domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt
- Acceptance:
  - `Collator::compare("a", "A")` and `("a", "á")` are Equal, `("a", "b")` is Less, at primary strength
  - the collator is built once from the environment locale and falls back to `en` when the locale is unavailable
  - sorting `["b", "A", "á", "c"]` by name gives `["A", "á", "b", "c"]` (stable on ties)

### T-13a-011 — UTF-8 byte size and byte-limited truncation
- Priority: P1
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - `byte_size("日")` is 3; `take_bytes("日本語", 4)` returns `日`
  - `take_bytes` never splits a character and keeps original U+FFFD characters (deviation D-14)

### T-13a-012 — Legacy HTML decode for tracker and catalogue text
- Priority: P1
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - `html_decode_legacy("Tom &amp; Jerry")` returns `Tom & Jerry`; `"a<br>b"` returns `a\nb`; `"&#8217;"` returns `’`
  - tags are removed and their text kept; `<script>` text is removed (test with a sample AniList title payload)

### T-13a-013 — Hash helpers and disk cache keys
- Priority: P0
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/lang/Hash.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `md5_hex(b"")` is `d41d8cd98f00b204e9800998ecf8427e`; `disk_key("abc")` is `900150983cd24fb0d6963f7d28e17f72`
  - `sha256_hex(b"")` starts with `e3b0c44298fc1c14`; outputs are lowercase and the right length
  - `sha1_hex` is available for the torrent info hash (T-13a-037)

### T-13a-014 — Filename sanitiser (FAT rules, NTFS-safe, byte and length budgets)
- Priority: P0
- Effort: M
- Depends: T-13a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt, plans/addenda/requirements-cross-platform.md
- Acceptance:
  - every vector in p1-edge-cases-and-tests.md section 6 matches, including `CON` becoming `_CON` (deviation D-1)
  - the component never exceeds 240 UTF-16 units and never ends in a lone surrogate (deviation D-2)
  - `page_filename("Naruto", "Ch 1", 7)` returns `Naruto - Ch 1 - 7`; the base name is cut by the 250-byte budget before the suffix is added

### T-13a-015 — Storage root folder and folder writability check
- Priority: P0
- Effort: S
- Depends: T-01a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/storage/AndroidStorageFolderProvider.kt, core/common/src/main/java/tachiyomi/core/common/storage/FolderProvider.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `FolderProvider::directory()` returns `Paths::downloads_default()` (`$XDG_DOWNLOAD_DIR/Kurobayomi`) and `url()` returns its `file://` URL
  - a writability probe creates and removes a temporary file in the chosen folder; a read-only folder shows the storage step error text
  - no code reads `XDG_*` outside kuro-platform (grep check)

### T-13a-016 — Disk usage, free and total space
- Priority: P0
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `free_bytes` uses `f_bavail * f_frsize` and `total_bytes` uses `f_blocks * f_frsize` via rustix statvfs; a missing path returns `None`
  - `dir_size` sums regular files, does not follow symlinks, and terminates on a symlink loop (test)

### T-13a-017 — Name and extension helpers on paths
- Priority: P1
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/storage/UniFileExtensions.kt
- Acceptance:
  - `extension_quirk("README")` returns `README` and `name_without_extension_quirk(".hidden")` returns the empty string (documented quirks, used only by ported UI text)
  - new call sites use `Path::extension`; grep shows the quirk helpers are used only in the ported UI text module

### T-13a-018 — Empty .nomedia marker in download folders
- Priority: P1
- Effort: S
- Depends: T-13a-015
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `create_nomedia(dir)` creates an empty `.nomedia` once and returns true; a second call returns false and leaves the file unchanged
  - a missing directory returns false without creating anything
  - the Linux media-scan step is a documented no-op (no call to any indexer)

### T-13a-019 — Temporary file manager
- Priority: P1
- Effort: S
- Depends: T-13a-015
- Sources: core/common/src/main/java/tachiyomi/core/common/storage/UniFileTempFileManager.kt
- Acceptance:
  - `TempFiles::create_from` names files with a prefix padded to three bytes and the `.tmp` suffix, inside `<cache>/tmp`
  - `clear()` removes the directory and a second call returns Ok
  - the file handle closes on drop (no leaked descriptor under `lsof` in a test loop of 100 creates)

### T-13a-020 — Copy an installed file read-only
- Priority: P1
- Effort: S
- Depends: T-13a-015
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt
- Acceptance:
  - `copy_read_only(src, dst, overwrite=false)` on an existing destination returns `DestinationExists`
  - the copy succeeds and the destination mode is `0o444` after the copy (deviation D-5); a failed copy leaves no read-only partial file
  - a directory source creates the destination directory and copies no content (documented parity, test asserts it)

### T-13a-021 — Image magic-byte sniffing and MIME and extension mapping
- Priority: P0
- Effort: M
- Depends: T-13a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - `sniff` matches every row of the table in p1-data-and-config.md section 2.4 and every sniff vector in p1-edge-cases-and-tests.md section 10.1
  - `is_image_name("x.JPG", no_sniff)` is false (case-sensitive extension); `is_image_name("x.jpeg", sniff_jpeg)` is true
  - `extension_for(Some("image/jpeg"), _)` returns `jpg`; `extension_for(None, unknown)` returns `jpg`

### T-13a-022 — Image decode service (formats, dimensions, JPEG q100 encode)
- Priority: P0
- Effort: M
- Depends: T-13a-021
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - `decode_rgba` decodes JPEG, PNG, GIF, WebP and JXL fixtures; HEIF and AVIF decode when the `heif` feature is enabled, otherwise return `Decode("feature disabled")`
  - `dimensions` reads headers without a full decode (under 1 ms for a 4000 px JPEG)
  - `encode_jpeg_q100` output decodes back to the same size; truncated input returns `Decode` without panic
  - spike result for libarchive binding and AV1 decoder recorded (OQ-13a-5, OQ-13a-6)

### T-13a-023 — Animated-image support rules
- Priority: P0
- Effort: S
- Depends: T-13a-021
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - `is_animated_supported` returns true for GIF and for animated WebP, false for still images, and false for animated HEIF and AVIF (documented capability change)
  - the reader calls it before the background decision; a GIF page never gets a computed background

### T-13a-024 — Wide-image (double-page) detection
- Priority: P0
- Effort: S
- Depends: T-13a-022
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt
- Acceptance:
  - `is_wide(1200, 800)` is true; `is_wide(800, 800)` is false (strict comparison)
  - detection uses header dimensions only (no full decode), verified by a test that uses a truncated image with valid headers

### T-13a-025 — Split a double-page spread in half
- Priority: P1
- Effort: M
- Depends: T-13a-022
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt
- Acceptance:
  - `split_in_half(img, Left)` and `(img, Right)` produce `W/2 x H` images matching the column ranges in behaviour 5.7 (vector: 5 x 1 Right gives columns 3 and 4)
  - the reader side selection table (four cases plus the invert rule) is a pure function with a unit test covering all eight combinations
  - output is JPEG quality 100

### T-13a-026 — Rotate a page a quarter turn for dual-page rotate-to-fit
- Priority: P1
- Effort: S
- Depends: T-13a-022
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt
- Acceptance:
  - `rotate_quarter(300x200, Cw90)` returns 200x300; `Ccw90` returns the reverse rotation
  - the caller's sign rule (`-90` when invert, else `90`) is a pure function with a test

### T-13a-027 — Merge a spread into one page (split and merge)
- Priority: P1
- Effort: S
- Depends: T-13a-022
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt
- Acceptance:
  - `split_and_merge(1000x600, Right)` returns 500x1200 with the right half on top; `Left` puts the left half on top
  - the webtoon side rule (`upper = Left` when invert, else `Right`) is a pure function with a test

### T-13a-028 — Split tall images at download time
- Priority: P1
- Effort: M
- Depends: T-13a-022, T-13a-014
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - `tall_split_plan` matches the three rows of p1-edge-cases-and-tests.md section 10.4 exactly (H=10000 gives 0-3333, 3333-6666, 6666-10000)
  - the trigger is `H / W > 3` with integer division (500x2000 splits, 500x1999 does not)
  - a failure removes the parts for the prefix and keeps the original file (test with a corrupted region); the caller ignores the boolean as in the Kotlin
  - setting `split_tall_images` (default true) gates the call

### T-13a-029 — Automatic reader page background (chooseBackground port)
- Priority: P1
- Effort: M
- Depends: T-13a-022
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt
- Acceptance:
  - `choose_background` reproduces the recorded outputs in `crates/kuro-core/tests/data/background_reference.json` for every synthetic image in p1-edge-cases-and-tests.md section 10.5
  - widths 100 to 109 do not panic and follow deviation D-6
  - the reader paints `Solid` as one colour and `VerticalBands` with four stops at 0, 1/3, 2/3 and 1 (verified by a screenshot test)

### T-13a-030 — Drop the hardware bitmap threshold and EGL texture limit
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - the preference `pref_hardware_bitmap_threshold` is not read by any Rust code; migration ignores it (documented in p1-data-and-config.md section 9)
  - the desktop reader does not show the texture-limit row; the drop is recorded in the release notes as "no user-visible loss: GTK manages textures"
  - the option list function from behaviour 7.2 is not ported unless a maximum-decode-size setting is later added (decision recorded)

### T-13a-031 — Archive listing and entry iteration (zip, cbz, rar, 7z, tar via libarchive)
- Priority: P0
- Effort: L
- Depends: T-13a-009, T-13a-021
- Sources: core/archive/src/main/kotlin/mihon/core/archive/ArchiveReader.kt, core/archive/src/main/kotlin/mihon/core/archive/ArchiveInputStream.kt, core/archive/src/main/kotlin/mihon/core/archive/ArchiveEntry.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ArchivePageLoader.kt
- Acceptance:
  - `ArchiveReader::open` memory-maps the file; `entries()` lists the fixture archives in archive order with names and `is_file`
  - the libarchive filter and format set matches `readSupportFilterAll` and `readSupportFormatAll` (fixture per format: zip, cbz with deflate, cbr via RAR, 7z, tar)
  - the page filter (image extension or sniff) and natural sort produce `1.jpg, 2.jpg, 10.jpg` for the fixture
  - 8 threads listing the same archive concurrently return identical results (no shared mutable state)

### T-13a-032 — Stream one archive entry by exact name
- Priority: P0
- Effort: S
- Depends: T-13a-031
- Sources: core/archive/src/main/kotlin/mihon/core/archive/ArchiveReader.kt, core/archive/src/main/kotlin/mihon/core/archive/ArchiveInputStream.kt
- Acceptance:
  - `open_entry("A.jpg")` returns None when the archive has only `a.jpg` (exact match)
  - a missing name returns None and frees the libarchive handle (no leak in a loop of 1000 lookups)
  - the reader reads the full entry bytes equal to the stored bytes (checksum test)

### T-13a-033 — EPUB page images (container, package, spine, manifest)
- Priority: P1
- Effort: M
- Depends: T-13a-031
- Sources: core/archive/src/main/kotlin/mihon/core/archive/EpubReader.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/metadata/EpubReaderExtensions.kt
- Acceptance:
  - the EPUB vectors in p1-edge-cases-and-tests.md section 9.2 and 9.3 pass (resolve, separators, package fallback, spine filter)
  - an `<image>` with `xlink:href` inside `<svg>` is found by prefix and local name (fixture `archive/sample.epub`)
  - backslash-separated EPUB (`META-INF\container.xml`) resolves with separator `\`; duplicate images are listed once (deviation D-10)

### T-13a-034 — CBZ writer for chapter downloads
- Priority: P1
- Effort: M
- Depends: T-13a-031, T-13a-014
- Sources: core/archive/src/main/kotlin/mihon/core/archive/ZipWriter.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `CbzWriter` writes stored entries named by basename, with mtime (clamped to 1980) and unix mode from the source
  - the output is written to `<name>.cbz_tmp` and renamed to `<name>.cbz` on finish (suffix `_tmp` from `MangaDownloader.kt:766`)
  - the output opens with `zip -T` and with the Rust reader from T-13a-031; entry count equals the file count

### T-13a-035 — Drop SAF, ParcelFileDescriptor and archive module plumbing
- Priority: P3
- Effort: S
- Depends: T-13a-031
- Sources: core/archive/src/main/kotlin/mihon/core/archive/UniFileExtensions.kt, core/archive/src/main/kotlin/mihon/core/archive/ArchiveReader.kt, core/archive/src/main/AndroidManifest.xml, core/archive/build.gradle.kts
- Acceptance:
  - no Rust code accepts a `content://` URI or a file descriptor for archives; `archive::open_path(&Path)` is the only entry point (grep check)
  - the user-facing loss is recorded: archives on Android SAF providers (cloud folders) must be copied to a local folder first
  - the module manifest and namespace have no Rust counterpart (documented)

### T-13a-036 — Bencode parser (strict grammar, unsigned key order)
- Priority: P2
- Effort: M
- Depends: T-13a-001
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/bencode/BencodeParser.kt, core/common/src/main/java/aniyomi/core/common/torrent/bencode/BencodeValue.kt
- Acceptance:
  - every row of the bencode table in p1-edge-cases-and-tests.md section 11.1 passes, including `i-0e`, `i03e`, `0:`, trailing data and duplicate keys
  - keys compare as unsigned bytes (deviation D-7): `d1:ai1e1:\x80i2ee` parses
  - integer overflow and string lengths above `i32::MAX` are errors

### T-13a-037 — Bencode encoder and info hash (SHA-1 of the info dictionary)
- Priority: P2
- Effort: S
- Depends: T-13a-036, T-13a-013
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/bencode/BencodeWriter.kt, core/common/src/main/java/aniyomi/core/common/torrent/TorrentHelpers.kt
- Acceptance:
  - `encode(parse(x)) == x` for both fixtures' info dictionaries
  - `sha1_hex(encode(info))` equals `104701de4b710ee9720ee249bf220384659e8c38` for the file fixture and `863eb76f1cd0643d9fd2eb4a7da866c3752991b5` for the directory fixture

### T-13a-038 — Torrent metadata extraction from a .torrent file
- Priority: P2
- Effort: M
- Depends: T-13a-037
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentHelpers.kt, core/common/src/main/java/aniyomi/core/common/torrent/model/Torrent.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/TorrentUtils.kt
- Acceptance:
  - the two fixture tests in p1-edge-cases-and-tests.md section 1 pass (title, hash, size, trackers in order with duplicates, file list with ids)
  - every structural error in section 11.2 returns `InvalidTorrent` (no NPE escapes)
  - `get_torrent_info` maps a magnet timeout to `ServerTimeout` and other magnet failures to `ServerUnavailable` (as `TorrentUtils.kt:28-33`)

### T-13a-039 — Torrent DTOs with TorrServer JSON field names
- Priority: P2
- Effort: S
- Depends: T-13a-038
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/model/Torrent.kt, core/common/src/main/java/aniyomi/core/common/torrent/model/TorrentRequest.kt
- Acceptance:
  - `TorrentRequest` for add serialises to `{"action":"add","link":"…","title":"…"}` (empty fields and `save_to_db` false omitted; test)
  - `TorrentDto` decodes `torrent_size` and `file_stats`, ignores unknown fields, and accepts a missing `file_stats`
  - the same DTO round-trips with the fixture response captured from a real TorrServer build (recorded in the test data)

### T-13a-040 — TorrServer HTTP client (echo, add, upload, stream URL)
- Priority: P2
- Effort: M
- Depends: area:12-network-stack, T-13a-039
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerApi.kt, core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerUtils.kt
- Acceptance:
  - every row of the client table in p1-edge-cases-and-tests.md section 11.4 passes against the axum test server
  - `echo` never errors (false on refused, 500 and timeout)
  - `stream_url` matches `http://127.0.0.1:{port}/stream/{name}?link={hash}&index={index}&play` with form-encoded name (test with `a b.mkv` giving `a+b.mkv`)

### T-13a-041 — TorrServer process supervisor (replaces the gomobile AAR and foreground service)
- Priority: P2
- Effort: L
- Depends: T-13a-040, T-01a-001
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerUtils.kt, core/common/src/main/java/aniyomi/core/common/torrent/DisabledTorrServerException.kt, app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt
- Acceptance:
  - `ensure_running` is single-flight: two concurrent calls start one child and return the same port (test)
  - readiness polls once per second for at most 11 polls then `ServerTimeout` (matches `wait(10)`); a child that exits early gives `Failed`
  - a configured port of `0` uses the port the server reports; `stop()` kills the child within 5 seconds; trackers are read from the preference at each start (deviation D-12)
  - the tracker delivery method is chosen after OQ-13a-1 is answered; the answer and the chosen method are recorded in this task's commit message

### T-13a-042 — Torrent preference keys, defaults and reset rule
- Priority: P2
- Effort: S
- Depends: T-13a-002, T-13a-004
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - the six keys `pref_torrserver_enable`, `pref_torrserver_shownotice`, `pref_torrserver_port` (default `"8090"`), `pref_torrserver_tackers` (sic), `pref_torrserver_proxymode` (default None), `pref_torrserver_proxyurl` (default empty) read with those defaults (test)
  - the default tracker list equals the 26-line value in p1-data-and-config.md section 6 (967 characters after removing spaces)
  - reset deletes the trackers key; the reset action is enabled only when torrents are on and the value differs from the default (test of the enable rule)
  - the first enable shows the legal notice once and sets `pref_torrserver_shownotice` (UI text from p1-ui-parity.md section 2.2)

### T-13a-043 — Torrent settings validators and proxy mode mapping
- Priority: P2
- Effort: S
- Depends: T-13a-042
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsTorrentScreen.kt, core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt
- Acceptance:
  - `validate_port` accepts exactly the vectors in p1-edge-cases-and-tests.md section 11.6 (`""` rejected, `0` and `65535` accepted, `65536` and `-1` rejected, `+80` accepted)
  - `validate_proxy_url` accepts `http, https, socks4, socks4a, socks5, socks5h` with a host and rejects `ftp://h`, `http:/h` and the empty string
  - the proxy mode sent to the server follows the decision for OQ-13a-3 (`None` sends no proxy mode, deviation D-13), tested against the stub server

### T-13a-044 — Torrent playback routing and play links
- Priority: P2
- Effort: S
- Depends: T-13a-040, T-13a-041
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerUtils.kt, core/common/src/main/java/aniyomi/core/common/torrent/DisabledTorrServerException.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `classify` routes `magnet:`, `*.torrent` and the local stream prefix to TorrServer only when torrents are enabled (vectors in section 11.6)
  - `magnet_index` parses `index=N` and returns 0 for absent or non-numeric values
  - the index question (OQ-13a-2) is answered by testing a multi-file torrent against the TorrServer build; the answer is recorded and the play link uses the 1-based id when required
  - a playback error is shown as a toast with the mapped message key (`torrserver_disabled` for `ServerUnavailable`)

### T-13a-045 — Drop Android vendor and browser checks (DeviceUtil)
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt
- Acceptance:
  - no Rust code checks MIUI, Samsung One UI, default browser lists or Android system properties (grep check)
  - if a consumer needs total memory, `kuro-platform::sys::total_memory_bytes()` reads `MemTotal` from `/proc/meminfo` and the 3 GiB threshold is a named constant (test with a fixture meminfo)
  - the drop reason is recorded: Android vendor quirks have no desktop equivalent

### T-13a-046 — Drop dp to px conversion
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/DensityExtensions.kt
- Acceptance:
  - no Rust helper converts dp to px; GTK sizes are logical pixels (grep check)
  - the reason is recorded: GTK applies the scale factor itself

### T-13a-047 — Drop the RxJava to coroutine bridge
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/tachiyomi/core/common/util/lang/RxCoroutineBridge.kt
- Acceptance:
  - no Rust code depends on an Rx crate or an Observable type (grep check)
  - the legacy Observable extension API is handled by the source area; the decision is recorded in the source area's task list

### T-13a-048 — Coroutine dispatch mapping (UI, IO, non-cancellable) onto tokio and glib
- Priority: P0
- Effort: S
- Depends: T-01a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt
- Acceptance:
  - `spawn_io` runs on the tokio blocking pool; `spawn_net` on the tokio runtime; `on_ui` posts to the glib main context (test from a worker thread)
  - non-cancellable work is tracked in a `JoinSet` on the app context and drained on shutdown (test: shutdown waits for a 200 ms task)
  - no GTK call happens off the main thread (checked by a debug assertion helper)

### T-13a-049 — Logging facade with logcat priority mapping
- Priority: P0
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/LogcatExtensions.kt
- Acceptance:
  - `priority_to_level` maps Verbose to TRACE, Debug to DEBUG, Info to INFO, Warn to WARN, Error and Assert to ERROR
  - the error chain is attached as structured fields (test: an error with a source prints both messages)

### T-13a-050 — Localised string formatting without backslash stripping
- Priority: P1
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt
- Acceptance:
  - a format string with `%s` and `%d` arguments returns the substituted text; a backslash in an argument is kept (deviation D-15)
  - plural selection uses the i18n crate's plural rules for English (`1 episode`, `2 episodes`)

### T-13a-051 — Empty JSON object constant
- Priority: P1
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/mihon/core/common/extensions/JsonObject.kt
- Acceptance:
  - `empty_object()` serialises to `{}` and the byte constant equals `b"{}"`
  - a test asserts both bytes `0x7B 0x7D`

### T-13a-052 — WebView helpers: user agent, version gate, default settings (WebKitGTK)
- Priority: P1
- Effort: M
- Depends: area:12-network-stack, T-13a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt
- Acceptance:
  - `infer_user_agent` returns the example value in p1-edge-cases-and-tests.md (`…Android 10; K)… Chrome/116.0.0.0…`) for the sample Android UA
  - `parse_chrome_major("Chrome/116.0.0.0")` is 116 and returns 0 when absent; the WebKitGTK version gate (proposal 2.44, to verify) is in kuro-platform
  - default settings enable JavaScript, local storage and third-party cookies on the WebKitGTK view; `get_html` returns decoded text (no JSON quoting); OQ-13a-4 answered and recorded

### T-13a-053 — In-app toast overlay
- Priority: P1
- Effort: S
- Depends: T-13a-048
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/ToastExtensions.kt
- Acceptance:
  - `toast::show(text, Short|Long)` displays for 2 s or 3.5 s, replaces the visible toast on a new call, and dismisses on click or Escape
  - the toast never takes keyboard focus (test: focus stays on the entry while a toast shows)
  - an empty text shows nothing rather than an empty box

### T-13a-054 — Drop the FFmpeg SAF string helper
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FFmpegUtils.kt
- Acceptance:
  - no Rust code builds a `content://` parameter for FFmpeg or mpv; paths go to mpv as plain strings with quotes escaped (test with a path containing a double quote)
  - the reason is recorded: Storage Access Framework does not exist on Linux

### T-13a-055 — Drop the FileProvider URI and cache share directory
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt
- Acceptance:
  - sharing an image uses the XDG portal (`kuro-platform::Opener`) with the file path; no `shared_image` cache directory is created (grep check)
  - the user-facing change is recorded: shared images are no longer copied into a cache folder first

### T-13a-056 — Drop external storage enumeration (SD card listing)
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - no Rust code lists mounted removable volumes for the download folder; the folder chooser handles selection (grep check)
  - the reason and the user loss (no quick SD card entry) are recorded

### T-13a-057 — Boolean to integer helper
- Priority: P1
- Effort: S
- Depends: T-13a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/util/lang/BooleanExtensions.kt
- Acceptance:
  - `i64::from(true)` is 1 and `i64::from(false)` is 0 (the helper is removed when no caller remains; the Rust code uses `i64::from` directly)
  - a grep check shows no `to_i64_bool` helper is left unused

### T-13a-058 — Port the torrent fixture tests and the area's test vectors
- Priority: P1
- Effort: S
- Depends: T-13a-009, T-13a-021, T-13a-036, T-13a-014
- Sources: core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt, core/common/src/test/resources/aniyomi/core/common/torrent/test-torrent-file.torrent, core/common/src/test/resources/aniyomi/core/common/torrent/test-torrent-directory.torrent
- Acceptance:
  - `crates/kuro-core/tests/torrent_meta.rs` has the two fixture cases with the expected values in p1-edge-cases-and-tests.md section 1
  - the fixtures are copied to `crates/kuro-core/tests/fixtures/torrent/` and their SHA-256 is recorded in the test file
  - `cargo test -p kuro-core` passes for every test vector named in p1-edge-cases-and-tests.md sections 2 to 11
