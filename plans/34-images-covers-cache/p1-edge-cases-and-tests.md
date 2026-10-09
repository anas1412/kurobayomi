# Area 34 — Images, covers and cache: edge cases, failure modes, tests

Companion to `p1-behavior.md` (cited "behavior §N"), `p1-data-and-config.md` ("data §N"), `p1-rust-design.md` ("design §N"), `p1-ui-parity.md` ("ui §N"), `p1-tasks.md` (T-34a-NNN). Kotlin paths are repo-relative to `/home/blackbox/aniyomi-src`. Test IDs: `C` = cover invalidation and keys, `G-` = golden tables, `F-` = fixtures, `E-IMG-` = edge cases.

Upstream tests: the repository has 8 `*Test.kt` files (`domain/src/test/...`, `app/src/test/java/mihon/core/migration/MigratorTest.kt`, `core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt`). None cover this area. No upstream test is ported; every test below is new.

---

## 1. Edge cases

Each row: scenario, what happens in Kotlin (OBSERVED), what Kurobayomi must do, and the task.

| ID | Scenario | Kotlin behavior | Kurobayomi requirement | Task |
|---|---|---|---|---|
| E-IMG-1 | Thumbnail URL is the empty string `""` | `getResourceType("")` → null → `Invalid image`; the cover file for `md5("")` is still computed by `getCoverFile` (behavior §18.3) | Treat empty as invalid for display; `cover_file` returns the `md5("")` path for compatibility only; never fetch | T-34a-017, T-34a-005 |
| E-IMG-2 | Thumbnail null but a custom cover file exists | Custom file wins (behavior §5.1 step 2) | Custom cover shown; key uses `manga;<id>;<lm>` | T-34a-018, T-34a-014 |
| E-IMG-3 | User picks a non-image file as cover | Bytes are copied as-is, no check (behavior §24.3); the next load fails | Validate with `sniff` before writing; on failure show `Failed to update cover` and keep the old file (proposal, changes Kotlin) | T-34a-050 |
| E-IMG-4 | Crash during a cover write | Partial file remains (non-atomic, behavior §8.3); next load decodes garbage and fails | Rust writes `<name>.part` and renames (design §6.4); on start, `.part` files older than 1 h are removed | T-34a-019 |
| E-IMG-5 | Stale `.part` or `.tmp` leftovers in cache folders | DiskLruCache `.tmp` files are not removed by `clear()` (behavior §20.6) | `clear()` removes them (design §6.3, D-11) | T-34a-038, T-34a-039 |
| E-IMG-6 | Chapter cache clear leaves `<key>.0.tmp` files and counts only entries | Count excludes temp files | Count = all files removed; document in the settings toast | T-34a-039 |
| E-IMG-7 | Destination size 0 in sampling | Integer division by zero throws `ArithmeticException` | Treat 0 as undefined (sample 1) (behavior §15.1, design §4.6) | T-34a-026 |
| E-IMG-8 | Two cover edits for the same entry at once | Both run on `launchIO`; last write wins, timestamps race | Per-entry async mutex (design §13.3) | T-34a-050 |
| E-IMG-9 | Disk cache written when `disk_cache` is WRITE disabled | `writeToDiskCache` does not check `writeEnabled` (behavior §9.1) | Check the policy before every disk write (Rust), documented as a fix | T-34a-020 |
| E-IMG-10 | `Custom-` prefix value from a source | Classified as HTTP (behavior §5.2) | Keep the classification for parity; log a warning; no producer exists | T-34a-017 |
| E-IMG-11 | `content://` URI (from an Android picker or backup) | Loaded through UniFile | Not supported on Linux: `Invalid image` (data §9.2, behavior §11.2) | T-34a-022 |
| E-IMG-12 | Reader buffer reused after the first decode | `BufferedSourceFetcher` wraps the live buffer; second read fails | Rust takes `Bytes` by value; a second decode of the same request returns `DecodeFailed` | T-34a-024 |
| E-IMG-13 | HTTP 304 with an empty body and no cache copy | Accepted (behavior §7.3); body read yields nothing; decode fails | Treat 304 without a stored copy as a network miss and refetch once without conditional headers (proposal) | T-34a-021 |
| E-IMG-14 | Anime, not in library: user edits cover | `editCover` does nothing for non-favourite remote entries and the UI still says "Cover updated" (behavior §19.3) | Show `Failed to update cover` (ui §5, U-1) | T-34a-050 |
| E-IMG-15 | Cover share repeated many times | Cover share uses `temp = true` without clearing the staging directory; files accumulate (behavior §21.7) | Clear staging before every share (design §8.3, D-14) | T-34a-045 |
| E-IMG-16 | `take_bytes` cuts a multi-byte UTF-8 sequence | `decodeToString(endIndex = n)` then strip `U+FFFD` (behavior §22.2) | Same result; the cut must not produce invalid UTF-8 in the file name | T-34a-043 |
| E-IMG-17 | Chapter numbers in an Arabic locale | ICU/JDK prints Arabic-Indic digits and an RTL mark on negatives (behavior §27.1) | ASCII digits (D-15); no RTL mark | T-34a-056 |
| E-IMG-18 | Reader opens a page while the cover file is being rewritten | Reader can read a truncated file during `writeAll` | Rust renames over the old file, so readers keep the old inode (design §6.4) | T-34a-019 |
| E-IMG-19 | Non-favourite manga with a custom cover file on disk (after removal from library) | `removeCovers` deletes the custom cover on removal (behavior §19.2); a file left behind is still used | On removal, delete custom and thumbnail files; on startup, custom covers of non-favourite entries are kept (they are only deleted by removal) | T-34a-012 |
| E-IMG-20 | Same key for two entries with `null` thumbnail URL and equal timestamps | Keys `manga;null;<lm>` collide; no image is fetched anyway | No cache entry is written for a null URL (`NoCoverSpecified` before the cache) | T-34a-014 |
| E-IMG-21 | Save while the staging folder is open in another viewer | `deleteRecursively` deletes the file that another window shows | Share staging uses a new unique name per share, and the previous file is removed only after 10 s (proposal) | T-34a-045 |
| E-IMG-22 | Save to Pictures when the Pictures folder does not exist | `mkdirs` creates it (behavior §21.5) | Same; if creation fails, `Error saving picture` | T-34a-044 |
| E-IMG-23 | Save when the Pictures folder is read-only | write throws; `error_saving_picture` (behavior §21.6) | Same message, error notification (behavior §23.2) | T-34a-044, T-34a-048 |
| E-IMG-24 | Very large image (e.g. 20000 x 20000) | Android downsamples or OOMs | Refuse above `max_pixels` with `Image too large` (design §4.6) | T-34a-027 |
| E-IMG-25 | Image with width or height 0 in header | `Failed to initialize decoder` (behavior §14.2) | Same texts in logs; placeholder in UI | T-34a-027 |
| E-IMG-26 | Animated GIF in a cover | First frame via Coil's GIF decoder (animated) | First frame in grids; animation only in the viewer (T-34a-033, P2) | T-34a-033 |
| E-IMG-27 | Page-list JSON corrupt | `decodeFromString` throws; caller falls back to network (behavior §20.3, §20.7) | Delete the file and return a miss (design §6.3, D-10) | T-34a-037 |
| E-IMG-28 | Page-list present but chapter URL changed upstream | Key differs; new list fetched | Same | T-34a-037 |
| E-IMG-29 | Two downloads of the same page image at once | DiskLruCache edit skipped for the second writer; the second download then reads the first's entry | In-flight set; second waits (design §6.2) | T-34a-038 |
| E-IMG-30 | Cancel while decoding | Coroutine cancelled; decoder not interruptible | Check the cancellation token before and after decode; drop result | T-34a-023 |
| E-IMG-31 | Source deleted after a cover request | `sourceManager.get` returns null → network client used (behavior §7.1) | Same fallback | T-34a-021 |
| E-IMG-32 | Settings change to `pref_display_profile_key` with an unreadable file | Silently ignored (behavior §14.6) | Same; no toast | T-34a-031 |
| E-IMG-33 | Disk full during a cache write | Editor aborted, exception re-raised (behavior §9.1); cover write deletes the partial file (behavior §8.3) | Same; no partial visible file | T-34a-019, T-34a-020 |
| E-IMG-34 | Clock skew (future `last_modified`) | `relativeTimeSpanString` returns `Just now` for negative differences (behavior §27.3) | Same | T-34a-058 |
| E-IMG-35 | Duration of exactly 60 s | `minutes = 1` (`Just now` only below 60 000 ms) | Same threshold | T-34a-058 |
| E-IMG-36 | Exception with a null message, class not in the list | Prints `ClassName: null` (behavior §28.1) | Same text for parity | T-34a-059 |

---

## 2. Race conditions

| ID | Interleaving | Outcome | Mitigation |
|---|---|---|---|
| R-1 | Cover fetch reads `covers/<md5>` while the same entry's cover is being replaced | Old or new file (non-atomic in Kotlin); can be partial | Rust rename-over (design §6.4) |
| R-2 | Two fetches for the same key write the disk cache concurrently | Kotlin: second `openEditor` returns null → `writeToDiskCache` returns null → unbuffered network stream | Rust: in-flight set, second waits (design §6.2) |
| R-3 | Removal from library runs `removeCovers` while a fetch for the cover is in flight | Fetch may recreate the cover file after deletion | Cover write checks favourite flag on completion; ignore write when entry no longer favourite (proposal) |
| R-4 | `awaitUpdateCoverLastModified` runs after the file is written but before the UI re-keys | UI shows the old image one frame | Harmless: key changes on next bind |
| R-5 | Share staging deleted while the previous share is open | Other app gets "file not found" | E-IMG-21 mitigation |
| R-6 | Page-list `put` from `recycle()` races with `get` of the same chapter in a new loader | `get` sees the old list or a miss | Both are safe; a miss refetches |
| R-7 | `clear()` runs while reader writes an image | Image write may fail (file removed mid-write) | Writer treats failure as miss; reader refetches (E-IMG-5 covers temp cleanup) |
| R-8 | Two covers edited for the same entry | Last write wins | Per-entry mutex (E-IMG-8) |
| R-9 | Decode permit held while the UI cancels | Permit held until decode ends | Decode is short; cancellation checked before `spawn_blocking` |

---

## 3. Test cases for invalidation and keys (`C`)

All inputs are constructed in memory; `paths` is a temp directory; `now_ms` is fixed by the test clock.

| ID | Setup | Call | Expected |
|---|---|---|---|
| C1 | remote `thumbnail_url = None` | `prep_update_cover(entry(url=A), remote(None), refresh_same_url=false)` | entry unchanged; no file deleted |
| C2 | remote `thumbnail_url = Some("")` | same | entry unchanged; no file deleted |
| C3 | remote `Some(A)`, entry url `A`, no custom | `refresh_same_url = false` | entry unchanged |
| C4 | remote `Some(A)`, entry url `A`, thumbnail file exists | `refresh_same_url = true` | thumbnail file deleted; `cover_last_modified = now_ms`; custom not present so the else branch runs |
| C5 | local entry (`source == 0`), remote `Some(B)` (different) | `refresh_same_url = false` | `cover_last_modified = now_ms`; no file deleted (isLocal branch first) |
| C6 | remote entry, custom cover exists, remote `Some(B)` | `refresh_same_url = false` | thumbnail file deleted; `cover_last_modified` unchanged; custom file kept |
| C7 | remote entry, no custom, remote `Some(B)` | `refresh_same_url = false` | thumbnail file deleted; `cover_last_modified = now_ms` |
| C8 | remote entry in library, `remove_covers` with thumbnail and custom both present | `remove_covers(entry, delete_custom = true)` | returns entry with new timestamp; delete count 2 |
| C9 | remote entry, no files present | `remove_covers` | returns the same entry (no timestamp change) (behavior §19.2) |
| C10 | local entry | `remove_covers` | returns the same entry; no deletion (isLocal early return) |
| C11 | custom cover absent | `delete_custom_cover(id)` | `false`; no error |
| C12 | keyer | `cache_key(art, custom_exists)` for: (a) custom exists, id 7, lm 100 → `manga;7;100`; (b) no custom, url `U`, lm 100 → `manga;U;100`; (c) no custom, url null → `manga;null;100`; (d) same as (b) with lm 101 → different key; (e) anime background with `use_background` and custom background → `anime;<id>;<background_lm>`; (f) anime background without custom → `anime;<background_url>;<background_lm>` | exact strings as listed |

Each case is one `#[test]` in `kuro-images/src/invalidate.rs` and `kuro-images/src/keys.rs`. Test C12 also checks that the Kotlin formats are equal by copying the expected strings from `p1-data-and-config.md` §6.3.

---

## 4. Failure modes and their texts

| Failure | Log text (Kotlin, behavior §) | Rust `ImageError` | User-visible |
|---|---|---|---|
| Empty or unknown URL | `Invalid image` (§11.3) | `InvalidImage` | placeholder icon |
| Null URL, no custom | `No cover specified` (§11.3) | `NoCoverSpecified` | placeholder icon |
| HTTP error | reason phrase (§7.3) | `Http(code, phrase)` | placeholder icon |
| 304 without cache (E-IMG-13) | `Null response source` if body null (§11.3) | `NullBody` | placeholder icon |
| Header zero | `Failed to initialize decoder` (§14.2) | `DecodeInit` | placeholder icon |
| Decoder returned no pixels | `Failed to decode image` (§14.4) | `DecodeFailed` | placeholder icon |
| Unsupported type (no feature) | Coil error | `UnsupportedFormat` | placeholder icon |
| Too large | (none) | `TooLarge` | placeholder icon |
| Cover write fails | `Failed to write response data to cover cache <name>` (§8.1) | `Io` | cover falls back to network copy |
| Snapshot promote fails | `Failed to write snapshot data to cover cache <name>` (§8.2) | `Io` | disk snapshot used |
| Page-list put fails | `Failed to put page list to cache` (§20.3) | `CacheError` | none |
| Clear item fails | `Failed to remove file from cache` (§20.6) | `Io` | toast `Error occurred while clearing` |
| Save not an image | `Not an image` (§21.4) | `NotAnImage` | `Error saving picture` |
| Save write fails | `Error saving picture` (§21.6) | `SaveFailed` | snackbar / notification |
| Cover edit fails | `Failed to update cover` (§23.1) | `Io` | snackbar |

---

## 5. Capabilities dropped or changed, with the user loss

| Capability | Kotlin | Kurobayomi | What the user loses |
|---|---|---|---|
| Screenshot blocking (`FLAG_SECURE`) | incognito and always-secure modes (behavior §30.1) | none | Screenshots and recordings are not blocked in incognito or always-secure mode. The setting stays and shows "Not available on Linux" (ui §12). |
| Hardware bitmaps and the device denylist | GPU bitmaps for large images (behavior §16) | RGBA textures | Nothing visible; memory use may be higher for very large covers. |
| Low-RAM RGB565 | `allowRgb565` on < 3 GiB (behavior §2.2) | always RGBA8 | Nothing visible on desktop hardware. |
| Media scan after save | `MediaScannerConnection` (behavior §21.5) | none | Images may not appear in Android gallery apps (no Linux equivalent needed). Files are in the Pictures folder. |
| MediaStore save (Q+) | (behavior §21.6) | plain file | Duplicate-name reuse behavior is replaced by overwrite (same visible result). |
| ICC display profile | applied to decodes and SSIV (behavior §14.6) | setting kept, no effect until `icc` feature | Colour-managed display for custom profiles. |
| Content URI covers | SAF picker result (behavior §11.2) | not supported | Covers that were stored as `content://` (none are produced on Linux). |
| Share sheet | Android chooser (behavior §23.4) | open in default viewer + copy URI | No app chooser; the default app opens the file. |
| Install-unknown-apps banner | Permissions.kt | none | No banner (not needed on Linux). |
| Fast-scroll animation workaround | Compose fix | none | No effect. |
| Assist URL | Android assist | none | No effect. |
| Shared-axis page transitions | Material motion | crossfade | The slide direction cue is gone; the crossfade is the GTK-native transition (ui §2.2). |

---

## 6. Golden tables (`G-`)

### 6.1 MD5 vectors (`G-MD5`), used by T-34a-002 and T-34a-069
| Input (UTF-8) | MD5 (lower-case hex) |
|---|---|
| `` (empty) | `d41d8cd98f00b204e9800998ecf8427e` |
| `abc` | `900150983cd24fb0d6963f7d28e17f72` |
| `1` | `c4ca4238a0b923820dcc509a6f75849b` |
| `https://example.invalid/covers/one.jpg` | `82437b03c2a148c30ab4597ab61a1d78` |
| `manga;https://example.invalid/covers/one.jpg;1700000000000` | `e2f3dd2a802b66728114818dd7e3ea6d` |
| `shared_image` | `e3b5554d5cecfbb32cc29f434a058731` |
| `chapter_disk_cache` | `9498aa7d3b71cc1a2addac211c0a1c66` |
Values computed with Python `hashlib.md5` in this session.

### 6.2 In-sample size (`G-SAMPLE`), used by T-34a-026 and T-34a-069
Formula: behavior §15.1. Destination sentinels: FILL undefined = `i32::MIN`, FIT undefined = `i32::MAX`. Results computed by a line-for-line port of the Kotlin formula (integer division, `takeHighestOneBit`, min or max, floor 1) in this session.
| Src (w x h) | Dst (w, h) | Scale | Expected sample |
|---|---|---|---|
| 1000 x 1500 | 500, 500 | Fit | 2 |
| 1000 x 1500 | 500, 500 | Fill | 2 |
| 1000 x 1500 | 300, 400 | Fit | 2 |
| 4000 x 6000 | 500, 750 | Fit | 8 |
| 4000 x 6000 | 500, 750 | Fill | 8 |
| 4000 x 6000 | undefined, undefined | Fill | 1 |
| 4000 x 6000 | undefined, undefined | Fit | 1 |
| 100 x 100 | 100, 100 | Fit | 1 |
| 1000 x 1000 | 600, 400 | Fit | 2 |
| 1000 x 1000 | 600, 400 | Fill | 1 |
| 5000 x 100 | 1000, 1000 | Fit | 4 |
| 5000 x 100 | 1000, 1000 | Fill | 1 |
| 2048 x 2048 | 1024, 1024 | Fit | 2 |
| 4000 x 6000 | 0, 0 (zero, E-IMG-7) | Fit | 1 (no panic) |
| Original (any) | Original | Fit | 1 |

### 6.3 Chapter and episode numbers (`G-NUM`), used by T-34a-056 and T-34a-069
Output of Java `DecimalFormat("#.###")` (HALF_EVEN, `.` separator), run in this session on JDK default locale `en`:
| Input | Output |
|---|---|
| 0.0 | `0` |
| 0.5 | `0.5` |
| 1.0 | `1` |
| 1.5 | `1.5` |
| 1.3333 | `1.333` |
| 1.0005 | `1` |
| 1.0015 | `1.002` |
| 1.0025 | `1.002` |
| 2.25 | `2.25` |
| -0.5 | `-0.5` |
| 10.0 | `10` |
| 0.0005 | `0` |
| 0.0004 | `0` |
| 100.1234 | `100.123` |
| NaN | `NaN` |
| +Infinity | `∞` |
Rust expects the same strings (ASCII digits; design §11). Negative zero: Rust output `0` (proposal, not verified in Java).
Locale note (observed, `ar`): Java prints `١.٥` for 1.5; the Rust table does not follow it (E-IMG-17).

### 6.4 Exception messages (`G-EXC`), used by T-34a-059 and T-34a-069
Inputs and expected texts (behavior §28.1; the online flag is given per case):
| Input | Online | Expected text |
|---|---|---|
| `DisabledTorrServerException` | any | `TorrServer is not running` |
| `HttpException(404)` | any | `HTTP 404, check website in WebView` |
| `UnknownHostException("example.invalid")`, offline | false | `No Internet connection` |
| `UnknownHostException("example.invalid")`, online | true | `Couldn't reach example.invalid` |
| `NoChaptersException` | any | `No results found` |
| `NoEpisodesException` | any | `No results found` |
| `SourceNotInstalledException` | any | `Source not found` |
| `AnimeSourceNotInstalledException` | any | `Source not found` |
| `IOException("disk full")` | any | `disk full` |
| `IOException` (null message) | any | `IOException` |
| `Exception("boom")` | any | `boom` |
| `IllegalStateException("bad")` | any | `IllegalStateException: bad` |
| `IllegalStateException` (null message) | any | `IllegalStateException: null` |

### 6.5 File names (`G-NAME`), used by T-34a-042, T-34a-043 and T-34a-069
| Input to `build_valid_filename` | Expected output | Note |
|---|---|---|
| `a/b:c*d` | `a_b_c_d` | FAT-forbidden characters |
| `  .hidden. ` | `hidden` | trim `.` and space |
| `` (empty) | `(invalid)` | |
| `...` | `(invalid)` | all trimmed |
| `a\u0001b` | `a_b` | control character |
| `con` | `_con` | NTFS reserved name (D-13); Android gives `con` |
| `CON.txt` | `_CON.txt` | reserved with extension (D-13) |
| `x` repeated 300 | `x` repeated 240 | `take(240)` in chars |
| `日本語` taken with `take_bytes(4)` | `日` | 4-byte cut keeps only the first full char (E-IMG-16) |
| `A`, chapter `B`, page 3, reader rule | `A - B - 3` | behavior §22.3 |
| title of 100 repetitions of `あ` (300 bytes), chapter empty, reader rule with page 1 | title cut to 246 bytes (82 characters), suffix ` - 1` (4 bytes), total 250 bytes | byte length of the result is 250 |

---

## 7. Fixtures (`F-`), used by T-34a-027, T-34a-028, T-34a-070

Stored under `kuro-images/tests/fixtures/`. Each file is small (under 100 KiB) and generated once with a script that is committed.

| ID | File | Content | Expected |
|---|---|---|---|
| F-01 | `jpeg_baseline_320x480.jpg` | baseline JPEG 320 x 480 | decodes, RGBA 320 x 480 |
| F-02 | `jpeg_exif_rot90.jpg` | JPEG 480 x 320 with EXIF orientation 6 | decodes; after orient: 320 x 480 (D-4) |
| F-03 | `png_rgba_100x100.png` | PNG with alpha gradient | decodes; alpha preserved |
| F-04 | `gif_static_64.gif` | GIF89a single frame | decodes |
| F-05 | `gif_animated_3f.gif` | GIF with 3 frames | decodes; first frame only in non-viewer path |
| F-06 | `webp_lossy_50.webp` | lossy WebP | decodes |
| F-07 | `bmp_24bit.bmp` | BMP 24-bit | decodes (Android decodes BMP; supported by `image`) |
| F-08 | `avif_still_96.avif` | AVIF still | decodes with feature `avif`; `UnsupportedFormat` without |
| F-09 | `jxl_still_96.jxl` | JPEG XL still | decodes with feature `jxl` |
| F-10 | `heic_still_96.heic` | HEIF still | decodes with feature `heif`; `UnsupportedFormat` without |
| F-11 | `truncated_jpeg.jpg` | first 1 KiB of F-01 | `DecodeFailed` or `DecodeInit` |
| F-12 | `zero_bytes.bin` | empty file | sniff returns none; as a cover: `UnsupportedFormat`; on save: `NotAnImage` |
| F-13 | `text_named.jpg` | text content named `.jpg` | `is_image(name)` true by extension, sniff false → cover decode `UnsupportedFormat`; save `NotAnImage` |
| F-14 | `zero_dimension.png` | PNG header with width 0 | `DecodeInit` |
| F-15 | `huge_header.png` | PNG header 30000 x 30000 | `TooLarge` (above `max_pixels`) |
| F-16 | `uppercase_ext.JPG` | valid JPEG, uppercase extension | `is_image` true by sniff (behavior §17.3) |
| F-17 | `jpeg_as_jpeg_ext.jpeg` | valid JPEG, `.jpeg` extension | `is_image` true by sniff only |

Fixture generation: Pillow or ImageMagick in a dev script; the script is `kuro-images/tests/fixtures/generate.py`; output checked in. Expected dimensions in the table are the test oracle.

---

## 8. HTTP tests (no network)

A local fixture server (`hyper`) with routes:
| Route | Response | Test |
|---|---|---|
| `/ok.jpg` | 200, F-01 | fetch and decode, source `Network` |
| `/nf.jpg` | 404, reason `Not Found` | `Http(404, "Not Found")` |
| `/nm.jpg` | 304, empty body, no cache | E-IMG-13: refetch without conditional headers, then decode |
| `/slow.jpg` | sleep 5 s then 200 | cancellation within 100 ms of token cancel (E-IMG-30) |
| `/headers.jpg` | 200, echoes request headers as JSON in a test-only header | source headers copied verbatim (behavior §7.2) |
| `/cc.jpg` | 200, echoes `Cache-Control` | `no-store` when network cache enabled; `no-cache, only-if-cached` when disabled |

---

## 9. Concurrency tests

| Test | Method | Pass condition |
|---|---|---|
| Fetch limit | 20 covers requested in parallel with a server that counts concurrent requests | max concurrent = 8 (behavior §2.3) |
| Decode limit | 20 decodes in parallel with an instrumented decoder | max concurrent = 3 |
| In-flight write | two `put_stream` for the same key | one write, second waits; file valid |
| Rename atomicity | reader opens file while writer renames over it | reader gets old or new full file, never partial (E-IMG-18) |
| Cancel | cancel during `put_stream` | `.part` removed; no value visible (behavior §9.1 step 6) |
| Cover edit mutex | two edits for same entry | serial order; final file equals second input |
| Share staging | two shares; first still open | first file exists until 10 s grace (E-IMG-21) |

---

## 10. Chapter-cache tests

| Test | Setup | Expected |
|---|---|---|
| Miss | empty cache | `get_page_list` returns `CacheError::Miss`; `is_image_cached` false |
| Round trip | put then get | equal `Vec<Page>` (index, url, imageUrl) |
| Default omission | page with `url = ""`, `imageUrl = None` | serialized as `{"index":n}` (data §6.2) |
| Unknown key tolerated | JSON with `"extra": 1` | parses (ignoreUnknownKeys) |
| Corrupt JSON | write `{` into the file | miss; file deleted (D-10, E-IMG-27) |
| Size limit | put 150 images of 1 MiB with limit 100 MiB | oldest removed; total ≤ 100 MiB |
| Clear counts | 3 pages, 4 images, 2 temp files | returns 9 (all files) (D-11, E-IMG-6) |
| Clear keeps covers | put cover in `<data>` and page image in `<cache>` | cover kept after clear |

---

## 11. Save and share tests

| Test | Input | Expected |
|---|---|---|
| Name for page | bytes = F-03, name `A - B - 3` | file `A - B - 3.png` (sniffed extension) |
| Name for cover | `CoverRgba` from F-01, name `Title` | file `Title.jpg`, JPEG quality 100 |
| Overwrite | same name twice | one file, second content |
| Folder per title | folder `Title/` | file under `Pictures/Kurobayomi/Title/` |
| No picture dir | `pictures_dir() = None` | `$HOME/Pictures/Kurobayomi` used |
| Non-image | F-13 | `NotAnImage`, nothing written |
| Read-only dir | chmod 555 | `SaveFailed`, no partial file (E-IMG-23) |
| Staging delete | previous share file exists | removed before the new write (cover path too, D-14) |

---

## 12. Formatting tests

| Test | Source of truth | File |
|---|---|---|
| `chapter_number_text` | §6.3 table | `kuro-domain/tests/format.rs` |
| `duration_text` | behavior §27.2 examples: 90061 s → `1d 1h`; 3660 s → `1h 1m`; 3630 s → `1h`; 330 s → `5m 30s`; 0 → fallback | same |
| `relative_time_text` | behavior §27.3: 0 → `Never`; now-30 s → `Just now`; future → `Just now`; now-60 s → relative | same; uses an injected clock |
| Relative date boundary | 7 days, 8 days | open question O-7; test asserts the Rust format pinned in the design |

---

## 13. Coverage matrix (scope file → tests)

| Scope file (`p1-inventory.md`) | Tests |
|---|---|
| ExceptionFormatter.kt | §6.4 |
| FastScrollAnimateItem.kt | none (dropped, T-34a-066) |
| ItemNumberFormatter.kt | §6.3, §12 |
| Navigator.kt | ui §2 (manual GTK test), T-34a-060 spike |
| Permissions.kt | none (dropped) |
| Resources.kt | ui §3.2 (visual test for error icon) |
| TimeUtils.kt | §12 |
| WindowSize.kt | T-34a-062 manual resize test |
| EditTextPreferenceExtensions.kt | none (dropped) |
| ViewExtensions.kt | ui §4.5 (menu order), T-34a-063 visible test |
| WindowExtensions.kt | none (dropped, §5) |
| AnimeBackgroundCache.kt | C11 and §11 (background paths) |
| AnimeCoverCache.kt | C8, C11 |
| ChapterCache.kt | §10 |
| MangaCoverCache.kt | C1-C11 |
| AnimeCoverKeyer.kt | C12 (e, f) |
| AnimeImageFetcher.kt | §8, C12 |
| BufferedSourceFetcher.kt | E-IMG-12 test |
| MangaCoverFetcher.kt | §8, C12 |
| MangaCoverKeyer.kt | C12 (a-d) |
| TachiyomiImageDecoder.kt | §7 fixtures |
| Utils.kt | G-SAMPLE §6.2 |
| ImageSaver.kt | §11 |

---

## 14. Manual test script (desktop)

1. Add a library manga with a cover; open the library grid offline (after the first load) and confirm the cover shows from `<data>/covers`.
2. Clear the chapter cache from Settings; confirm covers still show and the toast reads `Cache cleared, N files deleted`.
3. Open the cover viewer, press Escape, confirm it closes and the app stays.
4. Edit a custom cover with an F-01 file; confirm `Cover updated` and the new image; then Delete and confirm the thumbnail returns.
5. Pick F-13 (text named `.jpg`) as the cover; confirm `Failed to update cover` and the old image remains (E-IMG-3).
6. Save a reader page; confirm the `Picture saved` notification with a preview; click it and confirm the default viewer opens.
7. Save a cover twice; confirm one file, overwritten.
8. Resize the window across 720 px height/width; confirm tablet layout changes (ui §10).
9. Confirm incognito mode shows the "Not available on Linux" note for the secure-screen setting (ui §12).
10. Set the window to a 1 px width; confirm no panic and covers collapse without overflow.

---

## 15. Open items affecting tests

- O-1 (EXIF on Android): F-02 expectation is the Rust behaviour, not Android's. Re-check after a device comparison.
- O-7 (date format): the boundary test pins the Rust output, not Android's.
- O-5 (locale digits): the test pins ASCII.
