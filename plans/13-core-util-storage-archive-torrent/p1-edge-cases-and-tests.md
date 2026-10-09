# Area 13 — edge cases, failure modes, upstream tests, new tests

Every expected value below was produced either by running a line-by-line port of the Kotlin code (Python, during this plan), by computing the hash or regex directly, or by reading the source. Where a value is "expected Kotlin" it is what the Kotlin code returns; "expected Rust" is what the port must return (they differ only where the table says "deviation").

## 1. Upstream tests to port

| Upstream test | File | What it asserts | Port to | Fixture |
|---|---|---|---|---|
| `test_parseTorrentDetailsFromTorrentFileContent` (dynamic test, 2 cases) | `core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt` | parse result equals expected `Torrent` | `crates/kuro-core/tests/torrent_meta.rs` (two `#[test]`s) | `core/common/src/test/resources/aniyomi/core/common/torrent/test-torrent-file.torrent`, `test-torrent-directory.torrent` (copy to `crates/kuro-core/tests/fixtures/torrent/`) |

Expected results (from the test source, verified against the fixtures in this plan):

| Case | title | hash | size | trackers (in order) | files |
|---|---|---|---|---|---|
| file | `test-torrent-file` | `104701de4b710ee9720ee249bf220384659e8c38` | 18 | `udp://tracker.opentrackr.org:1337/announce` | `(None, "test-torrent-file", 18)` |
| directory | `test-torrent-directory` | `863eb76f1cd0643d9fd2eb4a7da866c3752991b5` | 12 | `udp://tracker.opentrackr.org:1337/announce`, `udp://tracker.opentrackr.org:1337/announce`, `https://tracker.tamersunion.org:443/announce`, `http://tracker.ipv6tracker.org:80/announce`, `udp://opentracker.io:6969/announce`, `udp://tracker.torrent.eu.org:451/announce` | `(1,"3/3.1",4)`, `(2,"2",2)`, `(3,"1",2)`, `(4,"3/3.2",4)` |

Upstream tests in other files that exercise this area's code are listed in section 9 (they live outside the scope and are noted only).

## 2. Natural order (`util/natural.rs`) — vectors

Expected Kotlin results (`CaseInsensitiveSimpleNaturalComparator`, sign of the result):

| a | b | sign(a,b) | Note |
|---|---|---|---|
| `file2` | `file10` | -1 | numeric run compares by value |
| `file10` | `file2` | 1 | |
| `a` | `B` | -1 | case-insensitive |
| `b` | `A` | 1 | |
| `01a` | `1b` | -1 | leading zeros numerically equal, then `a` < `b` |
| `1` | `a` | -1 | digit sorts before non-digit |
| `a` | `1` | 1 | |
| `Chapter 1` | `chapter 2` | -1 | case-insensitive, then numeric |
| `x9999999999999999999999` | `x1` | 1 | >19 digits wrap in 64-bit (9999…9 mod 2^64 is not 9999…9, compare unsigned) |
| `img001` | `img1` | 0 | numerically equal and both strings end together |
| `` (empty) | `a` | -1 | shorter prefix first |
| `a` | `` (empty) | 1 | |

Rust rule: the port must reproduce the wrapping `u64` arithmetic and unsigned compare. Test with `proptest`: for random ASCII strings of length <= 12 the result sign equals the Python reference implementation in section 2.1 (keep the reference as a test helper).

### 2.1 Reference (copy into the test helper)

```
fn natural_ref(a: &[u16], b: &[u16]) -> i32   // line-by-line port of AbstractSimpleNaturalComparator.compare
  with compareChars(c1,c2) = lower(c1) - lower(c2), lower = simple per-unit lowercase
```

## 3. Text helpers — vectors

| Function | Input | Expected | Note |
|---|---|---|---|
| `chop` | `"abcdef"`, 5, default `…` | `"abcd…"` (5 UTF-16 units) | `take(4) + "…"` |
| `chop` | `"abc"`, 5 | `"abc"` | not longer than count |
| `chop` | `"abcdef"`, 0, `"…"` | `Err(ReplacementTooLong)` | Kotlin throws `IllegalArgumentException` |
| `truncate_center` | `"abcdefghij"`, 7, `"..."` | `"ab...ij"` | `pieceLength = floor((7-3)/2) = 2` |
| `truncate_center` | `"abc"`, 7 | `"abc"` | not longer than count |
| `take_bytes` | `"日本語"`, 4 | `"日"` (3 bytes) | cut at char boundary |
| `take_bytes` | `"日本語"`, 9 | `"日本語"` | equal to length |
| `take_bytes` | `"a\u{FFFD}b"`, 2 | `"a"` plus the cut | deviation: Kotlin strips every U+FFFD in the whole result; Rust keeps original U+FFFD characters |
| `byte_size` | `"日"` | 3 | UTF-8 |
| `html_decode_legacy` | `"Tom &amp; Jerry"` | `"Tom & Jerry"` | entities |
| `html_decode_legacy` | `"a<br>b"` | `"a\nb"` | `<br>` to newline |
| `html_decode_legacy` | `"<b>bold</b>"` | `"bold"` | tags stripped |
| `html_decode_legacy` | `"&#8217;"` | `"’"` (U+2019) | numeric reference |

## 4. Collation (`util/collate.rs`)

| a | b | ICU primary (Rust) | Java PRIMARY (Kotlin) |
|---|---|---|---|
| `a` | `A` | Equal | Equal |
| `a` | `á` | Equal | Equal |
| `a` | `b` | Less | Less |
| `Ä` | `b` | Less | Less |

Test with `en` and with `de` (ß collation differences are expected and recorded, not failures).

## 5. Hash helpers (`util/hash.rs`)

| Input | md5 | sha256 (prefix) |
|---|---|---|
| `""` | `d41d8cd98f00b204e9800998ecf8427e` | `e3b0c44298fc1c14` |
| `"abc"` | `900150983cd24fb0d6963f7d28e17f72` | (compute in test) |

`disk_key("abc")` must equal `900150983cd24fb0d6963f7d28e17f72`.

## 6. Filename sanitiser (`storage/filename.rs`)

Expected Kotlin results (`DiskUtil.buildValidFilename`), and expected Rust results (after the Windows reserved-name rule, deviation D-1):

| Input | Kotlin | Rust | Note |
|---|---|---|---|
| `"  .hidden. "` | `hidden` | `hidden` | trims dots and spaces at both ends |
| `"a/b:c*d?"` | `a_b_c_d_` | `a_b_c_d_` | forbidden characters |
| `"..."` | `(invalid)` | `(invalid)` | empty after trim |
| `"name\u{1}x"` | `name_x` | `name_x` | control character |
| `"CON"` | `CON` | `_CON` | deviation D-1 (Windows reserved name) |
| `"CON.txt"` | `CON.txt` | `_CON.txt` | reserved name with extension |
| `"ＡＢＣ. "` | `ＡＢＣ` | `ＡＢＣ` | full-width letters kept |
| `"trail. "` | `trail` | `trail` | |
| `"a"` x 300 | 240 `a` characters | 240 `a` characters | component limit (UTF-16 units) |
| `"😀"` x 130 (260 UTF-16 units) | 120 emoji (240 units) | 120 emoji | the cut at unit 240 falls between pairs, so nothing splits |
| `"😀"` x 120 then `"a"` (241 units) | 120 emoji, `a` dropped | 120 emoji, `a` dropped | again no split; the surrogate-split case needs an odd boundary, covered by the next row |
| `"a"` x 239 then `"😀"` (241 units) | 239 `a` plus the emoji cut away (Kotlin `take(240)` keeps the high surrogate only) | 239 `a` (cut moved back one unit, deviation D-2) | the only input where the Kotlin output contains a lone high surrogate |

Rust rule for lone surrogates: the input is UTF-8 (`&str`), so lone surrogates cannot exist; the cut at unit 240 can only split a pair when the pair straddles the boundary. Move the cut back one unit in that case (deviation D-2, no visible change for the vectors above).

Page filename vectors (`storage::filename::page_filename`): title `"Naruto"`, chapter `"Ch 1"`, page `7`: base `"Naruto - Ch 1"` (13 bytes) fits in `250 - byte_size(" - 7") (4)` = 246, result `"Naruto - Ch 1 - 7"`.

## 7. Preferences (`prefs/*`) — semantic vectors

| Scenario | Kotlin result | Rust expected |
|---|---|---|
| Primitive `Int` pref, stored value is a string `"abc"` | `get()` returns default and deletes the key (`AndroidPreference.kt:72-80`) | same; the key is removed |
| Object (enum) pref, stored value `"Unknown"` | `get()` returns default; stored value kept (`enumValueOf` throws, `getEnum` maps to default) | same |
| Object pref with a deserializer that throws | default returned, stored value kept (`AndroidPreference.kt:222-228`) | same |
| `Pref<bool>.toggle()` from `false` | sets `true`, returns `true` | same |
| `Pref<BTreeSet>.insert("x")` | set contains `x` (new set value) | same |
| `isSet()` after `delete()` | `false` | `false` |
| `changes()` subscription | emits current value first (`ignition`), then each change, conflated | `watch` receiver starts with the current value; rapid writes collapse |
| `clear` (key `null`) | every registered pref re-reads | registry re-reads all keys |
| In-memory store `getStringSet` | throws `NotImplementedError` | implemented (deviation D-3) |
| In-memory `set` then a new `getString` from the same store | returns the initial value (defect) | returns the new value (deviation D-4) |
| `TriState::next()` | `Disabled -> EnabledIs -> EnabledNot -> Disabled` | same |
| `CheckState::next()` | `Checked -> None -> Checked` | same |
| `TriCheck::next()` | `None -> Include -> Exclude -> None` | same |

Key-prefix vectors: `is_private("__PRIVATE_x")` true; `is_private("x__PRIVATE_")` false; `private_key("pref")` = `"__PRIVATE_pref"`; `app_state_key("last_version_code")` = `"__APP_STATE_last_version_code"`.

## 8. Storage (`storage/*`)

| Function | Scenario | Expected |
|---|---|---|
| `copy_read_only(src, dst, false)` | `dst` exists | `Err(DestinationExists)` |
| `copy_read_only(src, dst, true)` | `dst` exists, deletable | copies; dst mode `0o444` (proposal, after the copy, deviation D-5) |
| `copy_read_only(src, dst, true)` | `dst` is a non-empty directory | `Err` (delete fails, as in Kotlin) |
| `copy_read_only(dir, dst, _)` | source is a directory | creates `dst` directory, copies no content (Kotlin behaviour kept; flagged) |
| `copy_read_only(missing, dst, _)` | source missing | `Err(SourceMissing)` |
| `create_nomedia(dir)` | dir exists, no `.nomedia` | creates empty file, returns `true` |
| `create_nomedia(dir)` | `.nomedia` exists | returns `false`, file untouched |
| `create_nomedia(missing)` | dir missing | returns `false` (Kotlin: `dir.exists()` check) |
| `dir_size(dir)` | nested files 3 + 5 bytes | 8 |
| `dir_size(dir)` with symlink loop | must terminate | terminates; symlinks not followed (deviation: Kotlin recurses) |
| `free_bytes("/nonexistent")` | path missing | `None` (Kotlin `-1L`) |
| `total_bytes(root)` | root filesystem | `Some(n)` with `n > 0` |
| `TempFiles::create_from` with name `"a"` | prefix padded to 3 | file name starts with `"a  "` (two spaces), suffix `.tmp` |
| `TempFiles::clear` twice | second call | `Ok(())` (NotFound ignored) |
| `extension_quirk("README")` | no dot | `"README"` (Kotlin quirk) |
| `name_without_extension_quirk(".hidden")` | leading dot | `""` (Kotlin quirk) |

## 9. Archives (`archive/*`)

### 9.1 Zip and libarchive

| Scenario | Expected |
|---|---|
| CBZ with `001.jpg`, `002.PNG` (uppercase), `notes.txt`, `dir/` | `entries()` lists all four; image filter (behaviour 4.3) keeps `001.jpg` (extension) and `002.PNG` only if sniffed (uppercase extension is not matched, sniff decides) |
| Entry names `10.jpg`, `2.jpg`, `1.jpg` | sorted natural: `1.jpg`, `2.jpg`, `10.jpg` |
| Nameless entry (pathname null) | skipped; iteration continues (deviation from Kotlin, which ends iteration) |
| Entry name not UTF-8 | skipped (`NonUtf8Name` only from direct lookup) |
| `open_entry("A.jpg")` when archive has `a.jpg` | `None` (exact, case-sensitive) |
| `open_entry` on a missing name | `None`; handle freed (no leak under valgrind or LSan) |
| Truncated zip (central directory missing) | `Err(Open)`; no panic |
| Archive of 0 bytes | `Err(Open)` |
| Encrypted zip entry | `Err` for that entry (libarchive reports; test with a fixture) |
| RAR (`.cbr`) via libarchive | entries listed (test fixture required, see section 12) |

### 9.2 EPUB path vectors (`resolve_zip_path`, `parent_directory`)

Kotlin results (separator `/`):

| base | rel | result | Note |
|---|---|---|---|
| `OEBPS` | `text/a.xhtml` | `OEBPS/text/a.xhtml` | |
| `OEBPS/text` | `../img/a.png` | `OEBPS/img/a.png` | `..` resolved |
| `OEBPS` | `../../../x` | `x` | `..` clamped at root |
| `` (empty) | `x` | `x` | |
| `OEBPS` | `/abs/x` | `/abs/x` | absolute kept, leading `/` kept (Kotlin returns the input unchanged) |
| `OEBPS` | `./a.png` | `OEBPS/a.png` | `.` removed |
| `OEBPS//x` | `a` | `OEBPS/x/a` | duplicate separators collapsed |

`parent_directory("OEBPS/content.opf", '/')` = `OEBPS`; `parent_directory("content.opf", '/')` = `""`.

Backslash EPUB: an archive with entry `META-INF\container.xml` gives separator `\`; `resolve_zip_path("OEBPS", "..\\x", '\\')` = `x`.

### 9.3 EPUB package vectors

| Scenario | Expected |
|---|---|
| `container.xml` with `<rootfile full-path="OEBPS/content.opf"/>` | package href `OEBPS/content.opf` |
| `container.xml` with `<rootfile/>` (no attribute) | fallback `OEBPS/content.opf` (deviation: Kotlin returns `""`) |
| `container.xml` missing | fallback `OEBPS/content.opf` |
| manifest item `media-type="application/xhtml+xml"` in spine | included |
| manifest item `media-type="text/html"` in spine | excluded (Kotlin filters on XHTML only) |
| spine `idref` not in manifest | skipped (`mapNotNull`) |
| page with `<img src="">` | skipped (deviation: Kotlin adds the base directory path) |
| page with `<image xlink:href="../i.jpg"/>` inside `<svg>` | `OEBPS/i.jpg` (requires prefix+local name match) |
| duplicate image in two pages | listed once (deviation: Kotlin lists twice) |

### 9.4 CBZ writer

| Scenario | Expected |
|---|---|
| `add_file` of `/x/001.jpg` | entry named `001.jpg` (basename), stored (method 0) |
| entry mtime | from source; clamped to 1980-01-01 00:00:00 if earlier |
| `finish` | renames `<name>_tmp` to `<name>`; returns final path |
| `create` on existing `<name>_tmp` | truncates (Kotlin `wt` mode) |
| zip readable by `zip -T` and by the Rust reader | yes |

## 10. Images (`image/*`)

### 10.1 Sniff vectors (exact bytes)

| Head bytes (first 12 shown, rest zero) | Expected |
|---|---|
| `FF D8 FF E0 …` | Jpeg, animated false |
| `89 50 4E 47 0D 0A 1A 0A …` | Png |
| `52 49 46 46 xx xx xx xx 57 45 42 50` (`RIFF….WEBP`), animated bit clear | Webp, animated false |
| same with VP8X flags animation bit set | Webp, animated true |
| `47 49 46 38 39 61 …` (`GIF89a`) | Gif, animated true |
| `00 00 00 0C 4A 58 4C 20 0D 0A 87 0A …` | Jxl |
| `FF 0A …` | Jxl |
| `00 00 00 18 66 74 79 70 68 65 69 63 …` (`ftyp` at 4, brand `heic` at 8) | Heif |
| `00 00 00 1C 66 74 79 70 61 76 69 66 …` (brand `avif`) | Avif |
| `00 00 00 0C 66 74 79 70` followed by zeros (brand not found) | none |
| first 31 bytes only (`FF D8 FF` then 28 zero bytes…, length 31) | Jpeg (the Kotlin buffer is 32 bytes; zero-padding applies) |
| empty input | none |

### 10.2 Dimensions and predicates

| Input | Expected |
|---|---|
| 1000 x 800 | `is_wide` false |
| 1200 x 800 | `is_wide` true |
| 800 x 800 | `is_wide` false (strict `>`) |

### 10.3 Split and merge

| Operation | Input W x H | Output | Rule |
|---|---|---|---|
| `split_in_half(Left)` | 1000 x 600 | 500 x 600 | columns 0..499 |
| `split_in_half(Right)` | 1000 x 600 | 500 x 600 | columns 500..999 |
| `split_in_half(Right)` | 5 x 1 | 2 x 1 | columns 3..4 (middle column 2 dropped) |
| `rotate_quarter(Cw90)` | 300 x 200 | 200 x 300 | |
| `split_and_merge(Right)` | 1000 x 600 | 500 x 1200 | top = right half; bottom = left half |
| `split_and_merge(Left)` | 1000 x 600 | 500 x 1200 | top = left half; bottom = right half |

### 10.4 Tall split plan (`tall_split_plan`, behaviour 5.10)

Verified with the Python port (matches the Kotlin `splitData` loop):

| H | optimal height | Parts (index, top, bottom) |
|---|---|---|
| 10000 | 4320 | (0, 0, 3333), (1, 3333, 6666), (2, 6666, 10000) |
| 4320 | 4320 | (0, 0, 4320) (one part, not split) |
| 4321 | 4320 | (0, 0, 2160), (1, 2160, 4321) |

Trigger: `H = 4W` (`H / W` integer = 4, `> 3`) splits; `H = 4W - 1` does not. Example: `W = 500`, `H = 2000` is split (`2000/500 = 4 > 3`); `W = 500`, `H = 1999` is not (`1999/500 = 3`). Test both.

Part names: `split_part_name("007", 0)` = `007__001.jpg`; `split_part_name("007", 999)` = `007__1000.jpg` (four digits past 999, same as `%03d`).

Failure behaviour: a decode error on part 2 removes parts 1 and 2 (names for this prefix only), keeps the source, and returns `Err`.

Skip rules: GIF (always animated-supported) is never split; a page already named `007__001.jpg` is skipped by the caller (`MangaDownloader.kt:593`).

### 10.5 Background (`image/background.rs`)

Synthetic images (all RGB, alpha 255). Expected results are the Kotlin algorithm's outputs (derivable by hand from behaviour 5.12; recorded here as the contract):

| Image | Expected |
|---|---|
| any image with W < 50 or H < 50 | `Solid(white)` |
| solid black 200 x 300 | `Solid(black)` (all six border samples dark and close: the "uniform border" branch returns the top-left colour) |
| solid white 200 x 300 | `Solid(white)` |
| 200 x 300, top half (rows 0..149) red `(200,30,30)`, bottom half blue `(30,30,200)` | six samples not all close: falls through; expected `Solid(white)` or bands by the rules; the test pins the value produced by the port of the Kotlin algorithm (generate the expected value once with the Kotlin logic in a debug harness, store it) |
| 200 x 300 with a black frame 10 px thick, white centre | `VerticalBands` or `Solid(black)` per the rules; pinned as above |
| width 100..109 (any height >= 50), black columns at the right edge | must not panic; the offset column outside the image is "not dark" (deviation D-6) |
| landscape flag true with dark image | `Solid(blackColor)` when `darkBG` |

Rule for the test suite: each synthetic image's expected output is generated once by a Kotlin-port harness that is kept in `crates/kuro-core/tests/data/background_reference.json`, so the Rust test compares against recorded outputs, not hand-guesses.

### 10.6 Decode and encode

| Scenario | Expected |
|---|---|
| decode a PNG with alpha, then JPEG q100 encode | JPEG has no alpha; output dimensions equal |
| decode truncated JPEG | `Err(Decode)`; no panic |
| decode HEIF when the `heif` feature is off | `Err(Decode("feature disabled"))` |
| `dimensions()` of a JPEG | header only (no pixel decode; measure: under 1 ms for a 4000 px image) |

## 11. Torrent helpers (`torrent/*`)

### 11.1 Bencode acceptance and rejection (`bencode::parse`)

| Input (bytes, shown as text) | Expected | Note |
|---|---|---|
| `i0e` | `Int(0)` | |
| `i-5e` | `Int(-5)` | |
| `i-0e` | error | `-0` rejected |
| `i03e` | error | leading zero |
| `ie` | error | empty integer |
| `i1` (EOF) | error | EOF |
| `0:` | `Bytes(b"")` | empty string |
| `3:abc` | `Bytes(b"abc")` | |
| `3:ab` | error | EOF inside string |
| `l` + `i1e` + `e` | `List([Int(1)])` | |
| `le` | `List([])` | |
| `d1:ai1e1:bi2ee` | `Dict({a:1, b:2})` | sorted |
| `d1:bi2e1:ai1ee` | error "Dictionary keys out of order" | unsorted (both Kotlin and Rust) |
| `d1:ai1e1:ai2ee` | error | duplicate key |
| `i1eX` | error "extra data" | trailing data |
| `x` | error "Unexpected value type" | unknown head |
| `d1:ai1e1:\x80i2ee` (keys `a` then byte `0x80`) | Rust: `Dict` (unsigned: `0x61 < 0x80`) | deviation D-7: Kotlin rejects with "Dictionary keys out of order" because the signed byte `0x80` (-128) compares below `a` |
| 9223372036854775808 integer (`i9223372036854775808e`) | error (Kotlin `NumberFormatException`, Rust overflow error) | |
| string length 2147483648 (`2147483648:` then EOF) | error | length cap `Int.MAX_VALUE` |

### 11.2 Torrent metadata vs Kotlin

| Input | Expected |
|---|---|
| no `info` key | error (Kotlin NPE, Rust `InvalidTorrent`) |
| `info` without `name` | error |
| `info` with both `length` and `files` | `InvalidTorrent` ("Invalid torrent file") |
| `info` with neither | `InvalidTorrent` |
| `announce` present as integer | `InvalidTorrent` (Kotlin ClassCast wrapped) |
| `name` with invalid UTF-8 | error (Kotlin `CharacterCodingException`; Rust `Decode`) |
| `files[i].path` empty list | file path `""` (Kotlin joinToString of empty list) |
| trackers with an empty string | kept as `""` (no filtering, both) |

### 11.3 Info hash invariance

For the two fixtures the re-encoded info dictionary SHA-1 equals the SHA-1 of the raw info slice (verified in this plan). Test: `sha1(encode(info_value)) == sha1(raw_info_slice)` for both fixtures.

### 11.4 TorrServer client (`torrent/client.rs`)

Run against a local `axum` server returning fixed JSON:

| Scenario | Expected |
|---|---|
| `echo()` with server answering 200 `ok` | `true` |
| `echo()` with connection refused | `false`, no error |
| `echo()` with HTTP 500 | `false` |
| `add("magnet:?xt=…", "Ep", false)` | request body is `{"action":"add","link":"magnet:?xt=…","title":"Ep"}` (no `save_to_db` because false, no empty fields) |
| `add` with save true | body contains `"save_to_db":true` |
| `upload(bytes, "t.torrent", false)` | multipart parts `file`, `save=false`, `title`; response decoded |
| response with unknown fields | ignored (`ignoreUnknownKeys`) |
| response with `file_stats` missing | `file_stats` `None` |
| timeout on `add` (socket timeout) | `TorrentError::ServerTimeout` |
| `add` while server down | `TorrentError::ServerUnavailable` (mapped from connection refused) |
| `stream_url` with fileStats id 2 path `3/3.1`, index 2, hash `abc` | `http://127.0.0.1:{port}/stream/3.1?link=abc&index=2&play` |
| `stream_url` name with space `a b.mkv` | `…/stream/a+b.mkv?…` (form encoding) |
| `stream_url` for single-file (id None) and index 0 | name = torrent title |

### 11.5 Supervisor

| Scenario | Expected |
|---|---|
| `ensure_running()` twice concurrently | one child process started; both calls return the same port |
| child exits before readiness | `Failed(reason)`; no hang |
| `ensure_running()` when server already answers `echo` | returns the port of the running server without spawning (Kotlin: no-op when echo non-empty) |
| readiness: 11 polls at 1 s | fails after about 11 s with `ServerTimeout` (matches `wait(10)`) |
| `stop()` on a stopped supervisor | `Ok(())` |
| app shutdown while running | stop requested, up to 5 s wait, then kill (proposal) |
| port `0` configured | server reports its port; the supervisor uses the reported port, not `0` |

### 11.6 Torrent preferences and routing

| Scenario | Expected |
|---|---|
| `validate_port("")` | error (Kotlin `toIntOrNull` null) |
| `validate_port("0")` | `Ok(0)` |
| `validate_port("65535")` | `Ok(65535)` |
| `validate_port("65536")` | error |
| `validate_port("-1")` | error |
| `validate_port("+80")` | `Ok(80)` (Kotlin `toIntOrNull` accepts a leading `+`) |
| `validate_proxy_url("socks5://h:1080")` | ok |
| `validate_proxy_url("socks5h://h")` | ok |
| `validate_proxy_url("ftp://h")` | error (unsupported protocol, scheme `ftp`) |
| `validate_proxy_url("http:/h")` | error (host null) |
| `validate_proxy_url("")` | error (scheme null) |
| `classify("magnet:?xt=urn:btih:…", base, true)` | `TorrServer` |
| `classify("https://x/y.torrent", base, true)` | `TorrServer` |
| `classify("https://x/video.mp4", base, true)` | `Direct` |
| `classify("magnet:…", base, false)` | `Direct` (feature off) |
| `magnet_index("magnet:?xt=…&index=3&x=1")` | 3 |
| `magnet_index("magnet:?index=abc")` | 0 (NumberFormatException → 0) |
| `magnet_index("magnet:?xt=…")` | 0 |

## 12. Test fixtures needed

| Fixture | Used by | Source |
|---|---|---|
| `torrent/test-torrent-file.torrent` | torrent_meta | copy from `core/common/src/test/resources/aniyomi/core/common/torrent/` |
| `torrent/test-torrent-directory.torrent` | torrent_meta | same |
| `archive/simple.cbz` (3 PNGs, one JPEG, one txt, natural-order names `1.jpg`, `2.jpg`, `10.jpg`) | archive tests | create with `zip` |
| `archive/deflate.cbz` (deflate-compressed entries) | reader test | create with `zip -9` |
| `archive/sample.cbr` | libarchive RAR path | a RAR created with `rar a` (test skipped when `rar` missing) |
| `archive/sample.epub` (container, OPF, 2 XHTML pages, SVG image with `xlink:href`) | epub tests | hand-written, zipped |
| `archive/backslash.epub` (entry `META-INF\container.xml`) | separator test | hand-written, zipped with Python `zipfile` writing backslash names |
| `archive/truncated.zip` | error test | first 100 bytes of a valid zip |
| `image/background/*.png` (solid, two bands, frame, width 100, width 109) | background tests | generated by a script and committed |
| `image/reference/background_reference.json` | background tests | generated once by the Kotlin logic (debug harness) |
| `image/sample.webp` (animated), `image/sample.heic` (optional, `heif` feature) | sniff and decode | commit small files |

## 13. Failure modes and how they surface

| Failure | Kotlin surface | Rust surface |
|---|---|---|
| Archive truncated | exception in reader, page error | `ArchiveError::Open`, page error state |
| Image decode fails in reader | `setError()` on the page (`PagerPageHolder.kt`, catch Throwable) | `ImageError::Decode`, same page error |
| Tall split fails | logged, original kept, return value ignored | `Err`, logged, original kept |
| TorrServer not running | `DisabledTorrServerException`, message "TorrServer is not running" | `TorrentError::ServerUnavailable`, same message key |
| TorrServer timeout on magnet | `DeadTorrentException` | `ServerTimeout` |
| Invalid .torrent | `RuntimeException("Invalid torrent file")` | `InvalidTorrent` |
| Disk full during copy | `IOException` | `io::Error` (`ENOSPC`), temp file removed |
| Read-only target | `setReadOnly` before copy | copy then chmod |

## 14. Concurrency and race conditions

| Race | Mitigation | Test |
|---|---|---|
| Two torrents started at once (two playback requests) | supervisor `ensure_running` is single-flight (one spawn) | 11.5 first row |
| Stop while starting | `stop()` cancels the readiness wait and kills the child | stop-during-start test |
| Preference changed while server runs (trackers) | trackers applied at next start; `setTrackersList` reads the current value (defect fixed: Kotlin caches at construction) | change trackers then restart: new list is sent |
| Tall split while the same page is downloaded again | caller serialises per chapter (downloader), split writes into a per-chapter temp dir | integration test in the downloader area |
| `TempFiles::clear` during download | clear only at startup | startup order test (area 01) |
| `watch` channel flood | conflation; receiver sees the latest value | 1000 writes, receiver sees the last value |
| Image decode memory peak | semaphore (`num_cpus`) | stress test with 8 large synthetic pages (memory under 1 GB) |
| Archive handle reuse | each operation opens a handle; no shared state | concurrent `open_entry` from 8 threads on the same archive |

## 15. Deviations (summary, all recorded in behaviour)

| ID | Kotlin | Rust | Reason | Behaviour section |
|---|---|---|---|---|
| D-1 | Windows reserved names pass through | prefix `_` | cross-platform rule 5 | 3.1 |
| D-2 | cut may split a surrogate pair | move cut back | no lone surrogates | 3.1 |
| D-3 | in-memory `getStringSet` throws | implemented | tests need it | 1.5 |
| D-4 | in-memory `set` does not persist in the store | write-through | tests need it | 1.5 |
| D-5 | read-only set before copy | set after copy | no read-only partial file | 3.8 |
| D-6 | out-of-range offset column throws | treated as not dark | crash on widths 100..109 | 5.12 |
| D-7 | keys compared as signed bytes | unsigned (BEP 3) | correct torrent parsing | 6.1 |
| D-8 | `EpubReader` empty `full-path` returns `""` | fallback `OEBPS/content.opf` | defect | 4.5 |
| D-9 | `img`/`image` without attribute adds directory path | skipped | defect | 4.5 |
| D-10 | duplicate image paths listed | deduplicated | defect | 4.5 |
| D-11 | nameless archive entry ends listing | skipped | defect | 4.1 |
| D-12 | tracker list read once at construction | read at each start | defect | 6.7 |
| D-13 | `None` proxy mode sends `tracker` | open question OQ-13a-3 | defect | 6.8 |
| D-14 | `takeBytes` strips all U+FFFD | keeps original characters | defect | 2.5 |
| D-15 | Kotlin backslash stripping in strings | not stripped | defect (i18n area) | 7.9 |
| D-16 | `dir_size` follows symlinks | no follow | loop safety | 3.4 |
| D-17 | `copyAndSetReadOnlyTo` directory copies no content | same (flagged) | parity; no caller copies directories | 3.8 |

## 16. Open questions (block the matching tasks)

| ID | Question | Owner task |
|---|---|---|
| OQ-13a-1 | TorrServer tracker delivery (AAR `addTrackers` has no documented HTTP equivalent in the repository) | T-13a-041 |
| OQ-13a-2 | Index `0` for multi-file torrents (1-based ids) | T-13a-044 |
| OQ-13a-3 | Proxy mode `None` mapped to `tracker` by the Kotlin | T-13a-043 |
| OQ-13a-4 | Linux user agent: keep the Android-spoof transform or not | T-13a-052 |
| OQ-13a-5 | AV1 decoder for AVIF and whether HEIF/AVIF are default features | T-13a-022 |
| OQ-13a-6 | libarchive binding choice (crate vs `libarchive3-sys`) after the spike | T-13a-031 |

## 17. Upstream tests outside this area that exercise it (noted, not ported here)

Upstream tests in other areas touch this code indirectly (for example chapter sorting tests and the reader tests). They are owned by their areas. This area's direct test is the one in section 1.

## 18. Manual test script (desktop, once the features exist)

1. Enable TorrServer, accept the notice, set port 0: status shows "Running on port N"; disable: status shows "Stopped".
2. Change the trackers, stop and start the server: the new list is used (check with the TorrServer log).
3. Open a magnet link with `index=2`: the second file plays.
4. Open a `.torrent` URL: the stream starts (no server needed for parsing).
5. Read a manga with a wide page and "split double pages" on: left and right halves appear in the right order.
6. Read a manga with a 105 px wide page: no error page; background is white or the border colour.
7. Download a chapter with a 4000 px tall page: the chapter folder has `NNN__001.jpg` to `NNN__00k.jpg` and no `NNN.jpg`.
8. Download a chapter with an unreadable tall page (corrupted file): original kept, chapter still completes.
9. Name a manga "CON": the download folder is `_CON` (deviation D-1).
10. Turn the Linux disk almost full: the download checks free space and reports the shortfall.
