# Topic: decoder selection, format matrix and sampling

Deep dive for area 34. Facts: `p1-behavior.md` §13-§17; rules for the Rust side: `p1-rust-design.md` §4; test fixtures: `p1-edge-cases-and-tests.md` §6.2 and §7.

## 1. Who decodes what (Kotlin, as shipped)

Two decoders can run for one image:
1. Coil's platform decoder (Android `BitmapFactory` and `ImageDecoder` under Coil 3.1.0), for every image the custom factory does not claim.
2. `TachiyomiImageDecoder` (native `tachiyomi.decoder.ImageDecoder`), when the request sets `customDecoder` or the sniffed type is AVIF, JXL or HEIF (`p1-behavior.md` §13.1).

Coil consults the factories in registration order, and `TachiyomiImageDecoder.Factory` is registered first (`App.kt:189`), so the custom factory decides first. Its `create` returns null for everything except the three types and the forced flag, which hands the request to Coil's built-in decoders.

## 2. Format matrix

| Format | Sniffed as | Kotlin decoder (default request) | Kotlin decoder (`customDecoder`) | Animated in Kotlin | Rust decoder | Rust animated |
|---|---|---|---|---|---|---|
| JPEG | `Jpeg` | Coil platform | native | no | `image` (jpeg) | no |
| PNG | `Png` | Coil platform | native | no | `image` (png) | no |
| GIF | `Gif` | Coil platform (Coil GIF decoder) | native | yes (Coil, viewer) | `image` (gif), first frame | P2 (T-34a-033) |
| WebP | `Webp` | Coil platform | native | animated only on API 28+ | `image` (webp), first frame | P2 |
| BMP | (not in the `ImageType` table) | Coil platform | native | no | `image` (bmp) | no |
| AVIF | `Avif` | native | native | no (stills) | `avif-decode` (feature `avif`) | no |
| JPEG XL | `Jxl` | native | native | no | `jxl-oxide` (feature `jxl`) | no |
| HEIF/HEIC | `Heif` | native | native | Coil animates HEIF only on API 30+ | `libheif-rs` (feature `heif`, off) | no |

Notes:
- BMP is not in `ImageType` (`ImageUtil.kt:106-114`), so the name check never accepts `.bmp` and sniffing returns null. Android still decodes BMP through its platform decoder when the request reaches it. Rust accepts BMP by extension and by its magic (`BM`) as a documented extension of the table (deviation to note in the test fixture F-07). Impact: none for covers; Kotlin users with `.bmp` covers see the image, Rust shows it too.
- Sniffing reads 32 bytes only. Magic values for `avif`/`heif` depend on the `ftyp` brand, which is within those 32 bytes for normal files (the `ftyp` box is at offset 0 and is small).
- The native decoder's HEIF brand list is unknown (O-4). Rust uses the list in `p1-rust-design.md` §4.4.

## 3. Sampling worked examples

Source 4000 x 6000 (portrait cover), target a 150-px-wide grid tile with Fit scale, so the target is (150, Int.MAX) when the height is undefined in Kotlin, or (150, 225) when it is known:

| Case | Width term | Height term | Fit result | Final |
|---|---|---|---|---|
| target (150, 225) | 4000/150 = 26 → 16 | 6000/225 = 26 → 16 | max = 16 | sample 16 |
| target (150, undefined) | 16 | 6000/MAX = 0 → 0 | max = 16 | sample 16 |
| target (undefined, undefined) | 0 | 0 | 0 | 1 (full decode) |
| original | 1 | 1 | 1 | 1 |

Decoded size with sample 16: `ceil(4000/16) x ceil(6000/16)` = 250 x 375. That is larger than 150 x 225 because the sample is a power of two that keeps the image at least the target size (E-IMG-24 applies to very large originals).

Fill scale with the same target uses `min`: (150, 225) gives min(16, 16) = 16 here; for a 4000 x 1000 source with target (150, 225): width 26 → 16, height 4 → 4; Fill = min = 4; Fit = max = 16.

## 4. Undefined and zero dimensions

- `Dimension.Undefined` maps to the sentinel of the chosen scale (`Int.MIN_VALUE` for Fill, `Int.MAX_VALUE` for Fit) (`Utils.kt:21-26`). Division of a positive source by either sentinel gives 0, which the floor turns into 1 (`p1-behavior.md` §15.1).
- `Pixel(0)` in Kotlin divides by zero (`ArithmeticException`). Rust treats 0 as undefined (E-IMG-7). The Kotlin path is unreachable in practice because callers never pass zero; the rule is recorded so the Rust code cannot panic.
- Negative inputs cannot come from `Size`; no rule is needed beyond the sentinel behavior.

## 5. Post-decode steps

1. EXIF orientation (Rust proposal D-4; Kotlin unknown, O-1). Applied before sampling so the sampled size matches the displayed orientation.
2. Border cropping (`crop_borders`): native library (O-2). Rust: crop rows and columns whose colour is within a tolerance of the corner colour, capped at 25 percent per side. This is a proposal, not parity; the test set compares against the Android output on fixtures F-01 and F-03 before the proposal is accepted (T-34a-030).
3. ICC profile (D-3): not applied by default.
4. Texture: RGBA8 `gdk::MemoryTexture` on the GTK thread (design §5.3).

## 6. Memory budget for decoded images

Decoded RGBA is `width * height * 4` bytes. A 250 x 375 cover is 375 000 bytes. A 4000 x 6000 original decoded at Original size is 96 MB, which is why the viewer is the only path that asks for Original and why the memory cache budget counts `rgba.len()` (design §6.1). The reader path uses `ViewSizeResolver`-style sizes and is governed by area:43-reader-core/area:44-reader-viewers.

## 7. Error mapping (decode)

| Condition | Kotlin text (log) | Rust variant | Cover display |
|---|---|---|---|
| width or height 0 from header | `Failed to initialize decoder` | `DecodeInit` | error icon |
| decoder returns no bitmap | `Failed to decode image` | `DecodeFailed` | error icon |
| sniffed type without feature | Coil unsupported (generic error) | `UnsupportedFormat` | error icon |
| pixels above `max_pixels` | (none; OOM on Android) | `TooLarge` | error icon |
| not an image at all (sniff none) | Coil: no decoder | `UnsupportedFormat` | error icon |

## 8. Test anchors

- Golden sample table: `p1-edge-cases-and-tests.md` §6.2 (15 rows, including zero and sentinel inputs).
- Fixtures F-01 to F-17 with the expected result of each.
- The sample table in §3 above is a worked example; the golden table is the test oracle.
