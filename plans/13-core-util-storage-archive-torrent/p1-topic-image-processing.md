# Topic — image processing pipeline (sniffing, decode, page operations, background)

Scope: `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt` (728 lines, the only large file in the area). The algorithms are in `p1-behavior.md` section 5; this file covers the pipeline design, the codec matrix, memory and threading, and how the reader and downloader use the results. Tasks: T-13a-021 to T-13a-029.

## 1. Responsibilities

| Responsibility | Kotlin entry point | Rust module | Used by |
|---|---|---|---|
| Format detection from bytes | `findImageType`, `getImageType` | `image::sniff` | archive page filter, downloader extension, saver, decoder selection |
| Extension and MIME mapping | `isImage`, `getExtensionFromMimeType`, `ImageType` | `image::sniff` | downloader, local sources, saver |
| Animated-support decision | `isAnimatedAndSupported` | `image::props` | reader, tall splitter |
| Header-only dimensions | `isWideImage`, `extractImageOptions` | `image::decode::dimensions` | reader, splitter |
| Whole-page decode | `ImageDecoder.decode` (native) | `image::decode::decode_rgba` | reader background, page operations |
| Page operations | `splitInHalf`, `rotateImage`, `splitAndMerge` | `image::ops` | reader (double page) |
| Tall split to parts | `splitTallImage`, `splitData` | `image::ops::split_tall` | downloader |
| Background colour | `chooseBackground` | `image::background` | reader |
| Hardware bitmap policy | `canUseHardwareBitmap`, `HARDWARE_BITMAP_UNSUPPORTED` | none (dropped) | Coil decoder (Android) |

## 2. Pipeline

```
bytes (page file or archive entry)
  -> sniff(first 32 bytes)                  -> ImageKind + animated flag, or "not an image"
  -> is_animated_supported?                 -> animated GIF/WebP: show frames, no background, no split
  -> dimensions(header)                     -> is_wide? -> double-page operations
  -> decode_rgba(full)                      -> RgbaImage (only when an operation needs pixels)
       -> split_in_half / rotate / split_and_merge -> JPEG q100 bytes
       -> choose_background(img, landscape)  -> Background (reader paints it)
  -> display: GTK texture from the (possibly re-encoded) bytes
```

Rules
- Sniff before decode: an extension is trusted only for the name check (`isImage`); the decoder selection always uses bytes.
- Header dimensions are read before any full decode (`is_wide`, tall check), so a non-operated page is never fully decoded just to be inspected.
- Re-encoding is done only when an operation changes pixels (split, rotate, merge). Plain pages are displayed from their original bytes.

## 3. Codec matrix (Linux)

| Format | Source decoder (Android) | Rust decoder | Crate | Notes |
|---|---|---|---|---|
| JPEG | native (libjpeg-turbo, cmyk handling via lcms) | `image` JPEG | `image` | CMYK JPEG: `image` returns an error for CMYK in some versions; test fixture needed; fallback to a message "unsupported CMYK JPEG" (proposal) |
| PNG | native (libpng) | `image` PNG | `image` | 16-bit PNG converted to 8-bit RGBA |
| GIF | native (`is GIF8`, animated always) | `image` GIF (first frame for still use; frames for animation) | `image` | animated GIF shown as animation by the reader area |
| WebP | native (libwebp, animated flag from header) | `image` WebP | `image` | animated WebP detected from `VP8X` animation bit |
| JPEG XL | native (libjxl) | `jxl-oxide` | `jxl-oxide` | pure Rust, no system library |
| HEIF | native (libheif, libde265/dav1d) | `libheif-rs` | system `libheif` (feature `heif`) | optional feature (OQ-13a-5) |
| AVIF | native (libheif with AV1) | `libheif-rs` or `image` AVIF | feature `heif` or `avif` | decoder choice deferred to the spike (OQ-13a-5) |

Colour management: the Android decoder converts the embedded ICC profile to sRGB (`cmsCreate_sRGBProfile`, `java_wrapper.cpp` `nativeNewInstance`). Proposal: convert embedded ICC profiles to sRGB with `lcms2` only when the profile is present and not sRGB; otherwise ignore the profile (most manga pages are sRGB). Recorded as a visible-difference risk for wide-gamut scans; test fixture with an Adobe RGB JPEG.

Border cropping: the native decoder supports `cropBorders` (`ImageDecoder.newInstance(stream, cropBorders)`), but this area's calls never pass it (`ImageUtil.kt:339`, default false). Not ported (no caller uses it in this area; the reader area may use it, not in scope).

## 4. Memory and threading

- Decode and encode run on the tokio blocking pool (`spawn_blocking`). A semaphore limits concurrent full decodes to `num_cpus` (T-13a-022 acceptance).
- Memory per page: RGBA8 at `W * H * 4` bytes; a 1000 x 10000 page is about 40 MB; a double-page operation holds the source and the result at once (about 2x).
- Tall split reads the whole file into memory once (`Buffer().readFrom`, `ImageUtil.kt:215`); the Rust version reads the file once and decodes once.
- No GPU bitmaps; GTK textures are created on the main thread from the final bytes.

## 5. The rules that change pixels (summary, exact in behaviour 5.7 to 5.10)

| Operation | Output size | Output format | Rule |
|---|---|---|---|
| split_in_half(side) | `floor(W/2) x H` | JPEG q100 | column range `[0, floor(W/2))` for Left, `[W - floor(W/2), W)` for Right |
| rotate_quarter(Cw90/Ccw90) | `H x W` | JPEG q100 | exact quarter turn, no resampling |
| split_and_merge(upper) | `floor(W/2) x 2H` | JPEG q100 | upper half on top as selected; the other on bottom |
| split_tall(prefix, optimal) | parts of about `optimal` height | JPEG q100 per part | triggered when `H / W > 3` (integer); `optimal = 2 * max(monitor width, height)` |

## 6. The background algorithm (summary)

Full pseudo-code in behaviour 5.12. Summary of the decision flow:
1. Sample the six border positions (top-left, top-centre, top-right, bottom-right, bottom-centre, bottom-left) at rows 5 and H - 5. If all six are non-white and each is close to its neighbour, return that top-left colour (uniform border).
2. Count white corners; more than two forces "not dark".
3. Scan four vertical columns (left, right, and the two offset columns) at 25 sample rows. Count dark runs (14+ for "black streak") and white runs (15+ for "white streak"), and totals of dark (> 22) and white (> 22) pixels.
4. Decide `darkBG`, the black colour (from the first dark corner), and whether the top and bottom borders are black or white streaks.
5. Landscape: return solid black or white.
6. Portrait: choose the two-band gradient (black top and white bottom, or the reverse) when the corners and streaks agree; otherwise solid white.

Output: `Background::Solid(Rgba)` or `Background::VerticalBands { top, bottom }`.

Known defect handled: the offset column for widths 100 to 109 (deviation D-6). Reference outputs: `crates/kuro-core/tests/data/background_reference.json` generated once with a Kotlin-port harness (T-13a-029).

## 7. Interactions with the reader and downloader (other areas)

- Reader (`PagerPageHolder.kt`): calls sniff and animation check, then `chooseBackground` only when `automaticBackground` and not animated; double-page rules call `split_in_half` and `rotate_quarter` (T-13a-025, T-13a-026); webtoon calls `split_and_merge` (T-13a-027).
- Downloader (`MangaDownloader.kt`): calls `findImageType` for the file extension after download, `getExtensionFromMimeType` for the response, and `splitTallImage` per page when `split_tall_images` is on (T-13a-028). The boolean from `splitTallImage` is ignored (parity).
- Saver (`ImageSaver.kt`): `findImageType` on the saved bytes to pick the extension (T-13a-021).

## 8. Failure policy

| Failure | Behaviour |
|---|---|
| sniff returns nothing | not an image: skipped in archive listing; error page in reader |
| decode error | page error state (reader); split returns error and cleans parts (downloader) |
| encode error | operation fails; page error state |
| background decode error | `Solid(white)` (no error shown) |
| animated, unsupported (HEIF/AVIF animation) | shown as still (first frame), no split |

## 9. Tests (summary; vectors in p1-edge-cases-and-tests.md section 10)

- Sniff table (every row, including the 31-byte buffer edge).
- Operations: sizes and column ranges for even and odd widths.
- Tall plan: the three rows, plus the trigger boundary (`H = 4W` split, `H = 4W - 1` not).
- Background: synthetic images and the recorded reference outputs; the 100 to 109 px width case must not panic.
- Codec fixtures: JPEG, PNG (16-bit), GIF (animated), WebP (animated and still), JXL, HEIF (feature on), CMYK JPEG (expected unsupported message).
- Memory: 8 concurrent decodes of 1000 x 10000 pages stay under 1 GB RSS.
