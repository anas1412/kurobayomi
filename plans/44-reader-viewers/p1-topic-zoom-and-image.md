# Topic: zoom, pan, tiling and image algorithms

Owner scope: `ReaderPageImageView.kt` and the image parts of `PagerPageHolder.kt` and `WebtoonPageHolder.kt`, plus the native algorithms they call (`ImageUtil.chooseBackground`, border crop in the image-decoder fork). Source of SSIV behaviour: `library/src/main/java/com/davemorrissey/labs/subscaleview/SubsamplingScaleImageView.java` at `tachiyomiorg/subsampling-scale-image-view@66e0db195d`. Source of the native border code: `library/src/main/cpp/borders.cpp` and `borders.h` at `tachiyomiorg/image-decoder@41c059e540`. Both were read in full for the functions cited.

Sections 3 and 5 are referenced from p1-behavior.md and p1-rust-design.md by number. Keep the numbering.

---

## 1. What the Kotlin asks SSIV to do (summary of B§10.1)

| SSIV call | Value | Effect |
|---|---|---|
| `setMaxTileSize(max)` | 2048 (`hardwareBitmapThreshold`) | largest tile edge in source pixels |
| `setDoubleTapZoomStyle(ZOOM_FOCUS_CENTER)` | 2 | double tap zooms about the touch point |
| `setPanLimit(PAN_LIMIT_INSIDE)` | 1 | image edges stay inside the view when larger than it |
| `setMinimumTileDpi(180)` | 180 | tile resolution floor (screen dpi terms) |
| `setMinimumDpi(1)` | 1 | sets a temporary `maxScale = avgDpi / 1`, overwritten by `setupZoom` |
| `setMinimumScaleType(type)` | 1..6 (pref) | minimum scale rule (§2.1) |
| `setDoubleTapZoomDuration(ms)` | pref × animator scale, min 1 | double-tap animation |
| `setCropBorders(bool)` | pref | decoder-side crop (§6) |
| `setImage(ImageSource.inputStream(...))` | stream | tiled decode from the stream |
| `setHardwareConfig(bool)` | `max(w,h) <= threshold` | GPU-backed bitmaps; not used in Rust |

Reader overrides after ready (`ReaderPageImageView.setupZoom`):
- `maxScale = scale * 5`
- `doubleTapZoomScale = scale * 2`
- centre per zoom start (§2.5)

---

## 2. Scale model

### 2.1 Minimum scale (`minScale()`, `SubsamplingScaleImageView.java:1907-1941`)

Inputs: view width/height `vw, vh`; image source width/height `sw, sh` (effective, after rotation and crop, which the reader does not use other than crop borders); padding (zero in the reader); extra space `vExtra*` (zero, because the reader never calls the extra-space API).

```
hPad = paddingLeft + paddingRight + hExtra   (hExtra = round(left+right extra), zero here)
vPad = paddingTop + paddingBottom + vExtra
switch minimumScaleType:
  CENTER_INSIDE (1), default: min((vw - hPad) / sw, (vh - vPad) / sh)
  CENTER_CROP   (2):          max((vw - hPad) / sw, (vh - vPad) / sh)
  FIT_WIDTH     (3):          (vw - hPad) / sw
  FIT_HEIGHT    (4):          (vh - vPad) / sh
  ORIGINAL_SIZE (5):          1
  SMART_FIT     (6):          sh > sw ? (vw - hPad)/sw : (vh - vPad)/sh
  CUSTOM        (7):          minScale field
```

Division is float (`float` in Java), so ratios are computed in `f32` for exact parity with the observed rounding. Use `f32` in `zoom::scale::min_scale`.

Reader mapping from preference value (`pref_image_scale_type_key`, B§1.3): 1 → CENTER_INSIDE ("Fit screen"), 2 → CENTER_CROP (labelled "Stretch"), 3 → FIT_WIDTH, 4 → FIT_HEIGHT, 5 → ORIGINAL_SIZE, 6 → SMART_FIT. The mismatch between the label "Stretch" and CENTER_CROP is kept (open question 2 in B§15).

### 2.2 Limiting

`limitedScale(t) = min(maxScale, max(minScale(), t))` (`SubsamplingScaleImageView.java:1945` region). The max is `5 * fit` after `setupZoom`. Before ready, the max is the temporary dpi value.

### 2.3 Initial state on ready

On ready SSIV sets scale to `minScale()` and centre to the image centre (SSIV default; the reader's `landscapeZoom` check `scale == minScale` relies on it). `setupZoom` then applies the zoom start (§2.5).

### 2.4 Double tap target (B§10.5)

```
dt = min(maxScale, doubleTapZoomScale)            // 2 * fit, capped at max
zoomIn = scale <= dt * 0.9 || scale == minScale
target = zoomIn ? dt : minScale()
animate(scale -> target, centre -> touch point) duration = doubleTapZoomDuration, not interruptible
```

### 2.5 Zoom start (`ReaderPageImageView.setupZoom`, lines 276-287)

After ready, with the current `scale`:
- LEFT: `setScaleAndCenter(scale, (0, 0))`
- RIGHT: `setScaleAndCenter(scale, (sWidth, 0))`
- CENTER: `setScaleAndCenter(scale, center)` where `center` is the current centre (image centre)
- null config: no change

Because `scale` equals `minScale` at this point, the visible effect is a centre change only when the image is larger than the view on some axis.

---

## 3. Translation limits (`fitToBounds`)

SSIV keeps the translation inside bounds after every change (pinch, pan, animation frame). Reproduce exactly for `PAN_LIMIT_INSIDE`, which is the reader's mode.

Source: `SubsamplingScaleImageView.java:1325-1380` (`fitToBounds(boolean center, ScaleAndTranslate sat)`).

Inputs: `scale = limitedScale(sat.scale)`, `scaledW = scale * sw`, `scaledH = scale * sh`, and extra space `e* = 0` in the reader (`extra = panLimit == PAN_LIMIT_INSIDE` but all `vExtraSpace*` are zero).

Step A (lower bounds):
```
if center:
    tx = max(tx, vw - scaledW - extraRight)    // extraRight = 0
    ty = max(ty, vh - scaledH - extraBottom)   // extraBottom = 0
else:
    tx = max(tx, -scaledW - extraRight)
    ty = max(ty, -scaledH - extraBottom)
```

Step B (upper bounds), with `xRatio = 0.5` and `yRatio = 0.5` in the reader (zero padding gives the 0.5 default):
```
if center:
    maxTx = max(extraLeft, (vw - scaledW) * xRatio)    // extraLeft = 0
    maxTy = max(extraTop,  (vh - scaledH) * yRatio)
else:
    maxTx = max(extraLeft, vw)
    maxTy = max(extraTop,  vh)
tx = min(tx, maxTx)
ty = min(ty, maxTy)
```

Consequences (proved from the formulas, matches observed behaviour):
- `center = true`, image larger than view on x: `tx` ends in `[vw - scaledW, 0]`. Normal pan clamp.
- `center = true`, image smaller than view on x (`vw - scaledW = d > 0`): step A gives `tx >= d`; step B caps at `max(0, d * 0.5)`; result `tx = d / 2`. The image is centred horizontally.
- `center = false`: `tx` ends in `[-scaledW, vw]`; the image can be pushed nearly off-screen.

Vertical is identical with `vh`, `scaledH`.

`PAN_LIMIT_OUTSIDE` and `PAN_LIMIT_CENTER` are not used by the reader. Do not implement them.

Which `center` flag is used: SSIV passes `center = true` for zoom end and most touch paths, and `center = false` during quick scale and fling. Rust: pass `center = true` after a pinch or double-tap animation completes and `center = false` during a pan drag. This follows the SSIV call sites in `onTouchEvent` (`fitToBounds(true)` in the pinch branch, B§10.4).

### 3.1 Pan remaining (used for navigate-pan)

`getPanRemaining(RectF)` (`SubsamplingScaleImageView.java:2025-2047`), `PAN_LIMIT_INSIDE` branch:
```
scaledW = scale * sw; scaledH = scale * sh
left   = max(0, -tx)
right  = max(0, (scaledW + tx) - vw)
top    = max(0, -ty)
bottom = max(0, (scaledH + ty) - vh)
```
`canPanRight = right > 1`; `canPanLeft = left > 1` (`ReaderPageImageView.kt:196-212`).

`panRight()` target centre: `centre.x + vw / scale` (source units), animate with `animateCenter`, 250 ms, EASE_OUT_QUAD, interruptible (B§10.4). `panLeft()`: `centre.x - vw / scale`.

---

## 4. Pinch, quick scale and tiling

### 4.1 Pinch

Two pointers at `t0`, `t1`:
```
distStart, scaleStart, centreStart, translateStart  // captured on pointer down (second pointer)
distNow = dist(p0, p1); centreNow = midpoint
zoom if centreMoved > 5 || |distNow - distStart| > 5 || panning
scale = min(max, (distNow / distStart) * scaleStart)
if scale <= minScale(): distStart = distNow; scaleStart = minScale(); centreStart = centreNow; translateStart = translate
else (pan enabled):
    left0  = centreStart.x - translateStart.x     // source point under the start centre (view space offset)
    top0   = centreStart.y - translateStart.y
    leftN  = left0 * (scale / scaleStart)
    topN   = top0  * (scale / scaleStart)
    translate.x = centreNow.x - leftN
    translate.y = centreNow.y - topN
    if crossing from < view to >= view on an axis: fitToBounds(true); reset starts to current
fitToBounds(true)
refresh tiles
```

Identical logic is used by the webtoon strip through `WebtoonZoom::apply_pinch` in simplified form (B§10.7).

### 4.2 Quick scale (one-finger, after double tap)

Source: `SubsamplingScaleImageView.java`, quick-scale start at lines 480-492 and move handling at 700-746.

State at quick-scale start: `lastDist = -1`, `moved = false`, `sCentre = viewToSource(centreStart)`, `yStart = y`, `yLast = y`.

Each one-finger move while quick scaling (B§10.4):
```
dist    = |yStart - yNow| * 2 + quickScaleThreshold          // threshold = 20 dp in px
if lastDist == -1: lastDist = dist                             // first move only
upwards = yNow > yLast                                         // exact comparison kept from source
yLast   = yNow
span    = |1 - dist / lastDist| * 0.5
if span > 0.03 or moved:
    moved = true
    multiplier = lastDist > 0 ? (upwards ? 1 + span : 1 - span) : 1
    scale = clamp(scale * multiplier, minScale(), maxScale)
    translate: x = centreStart.x - (centreStart.x - translateStart.x) * (scale / scaleStart)   (same for y)
    if an axis crosses from smaller-than-view to >= view:
        fitToBounds(true); centreStart = sourceToView(sCentre); translateStart = translate; scaleStart = scale; dist = 0
fitToBounds(false)                                             // quick scale uses center = false
lastDist = dist                                                // updated on every move, including after the reset to 0
```
Two details that are easy to miss and are pinned by T-44a-080: `lastDist` is updated on every move (not only the first), and after a threshold crossing `dist` is 0, so the next step uses `multiplier = 1` (because `lastDist > 0` is false) and resets `lastDist` to the new `dist` on the following event.

### 4.3 Tiles

Tiling reproduces "tiles are decoded at the level of detail the current scale needs, capped at `maxTileSize`". The Rust version:
- Precomputes a pyramid: levels `L0` (full), `L1` (1/2), ... down to the level where the whole image fits in one `maxTileSize` tile (about 8 levels for a 16k page).
- At each frame, pick the level whose pixels-per-screen-pixel ratio is closest to 1 and at least `minimumTileDpi` (180 dpi equivalent: `level_scale >= screen_dpi / 180 * something`; implement as "never pick a level coarser than the display requires for 180 dpi", i.e. `tile_px_per_view_px <= 1` at the minimum-tile dpi).
- Upload only visible tiles. Free tiles outside the view plus one tile border.

Proposal: keep the pyramid simple (power-of-two box filter, computed on the decode worker). Document the simplification in the code; the upgrade path is per-tile decode from the source (`image` crate region decode is not available for all formats, so the pyramid is the portable choice).

Texture cap: 2048 px per tile edge (B§10.1).

---

## 5. Automatic background: `ImageUtil.chooseBackground` (port-ready)

Source: `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:338-580` (read in full; helper functions at 582-589).

Input: decoded RGBA image `img` (w × h), `landscape: bool` (widget aspect, §B9.3 step 11).

Constants: `TOP = 5`, `BOT = h - 5`, `LEFT = (w as f64 * 0.0275) as i32`, `RIGHT = w - LEFT`, `MID_X = w / 2`, `MID_Y = h / 2`, `OFFSET_X = (w as f64 * 0.01) as i32`, `LEFT_OFF = LEFT - OFFSET_X`, `RIGHT_OFF = RIGHT + OFFSET_X`, `WHITE = (255,255,255,255)`, `BLACK = (0,0,0,255)`.

Helpers:
```
is_dark(c)  = c.r < 40 && c.g < 40 && c.b < 40 && c.a > 200
is_white(c) = c.r + c.g + c.b > 740          // ints, not averaged
is_close(a, b) = |a.r-b.r| < 30 && |a.g-b.g| < 30 && |a.b-b.b| < 30
```

Algorithm:
1. If decode fails: return `Solid(WHITE)`. If `w < 50 || h < 50`: `Solid(WHITE)`.
2. Sample eight pixels: `tl = img[LEFT, TOP]`, `tr = img[RIGHT, TOP]`, `ml = img[LEFT, MID_Y]`, `mr = img[RIGHT, MID_Y]`, `tm = img[MID_X, TOP]`, `bl = img[LEFT, BOT]`, `bm = img[MID_X, BOT]`, `br = img[RIGHT, BOT]`. Let `dX` denote `is_dark(X)` for each.
3. `darkBG = (dTL && (dBL || dBR || dTR || dML || dTM)) || (dTR && (dBR || dBL || dMR || dTM))`.
4. Ring test: `ring = [tl, tm, tr, br, bm, bl]`. If for all i, `!is_white(ring[i]) && is_close(ring[i], ring[(i+1) % 6])` → `Solid(tl)`.
5. `whiteCorners = count(is_white over [tl, tr, bl, br])`. If `whiteCorners > 2` → `darkBG = false`.
6. `blackColor` = first of `tl`, `tr`, `bl`, `br` that is dark, else `WHITE`. (Order: tl, tr, bl, br.) Note: the source checks `topLeftIsDark -> topLeftPixel`, `topRightIsDark -> topRightPixel`, `botLeftIsDark -> botLeftPixel`, `botRightIsDark -> botRightPixel`.
7. Column scan. Counters: `overallWhite = 0`, `overallBlack = 0`, `topBlackStreak = 0`, `topWhiteStreak = 0`, `botBlackStreak = 0`, `botWhiteStreak = 0`. For `x` in `[LEFT, RIGHT, LEFT_OFF, RIGHT_OFF]` (stop early via `break outer`):
   - `notOffset = (x == LEFT || x == RIGHT)`.
   - `whiteRun = 0; white = 0; blackRun = 0; black = 0; blackStreak = false; whiteStreak = false`.
   - For `(index, y)` in `(0 .. h).step_by(h / 25)` (step = `h / 25` integer; if 0, Kotlin range step 0 throws; in Rust use `max(1, h/25)` and note that Kotlin never hits 0 because h >= 50):
     - `pixel = img[x, y]`; `pixelOff = img[x + (x < w/2 ? -OFFSET_X : OFFSET_X), y]`.
     - if `is_white(pixel)`: `whiteRun++; white++; if notOffset: overallWhite++;` if `whiteRun > 14`: `whiteStreak = true`; if `whiteRun > 6 && whiteRun >= index - 1`: `topWhiteStreak = whiteRun`.
     - else: `whiteRun = 0`; if `is_dark(pixel) && is_dark(pixelOff)`: `black++; if notOffset: overallBlack++; blackRun++; if blackRun >= 14: blackStreak = true; continue` (skips the streak-reset below).
     - (not continue) `if blackRun > 6 && blackRun >= index - 1: topBlackStreak = blackRun`; `blackRun = 0`.
   - After the row loop: `if blackRun > 6: botBlackStreak = blackRun` else `if whiteRun > 6: botWhiteStreak = whiteRun`.
   - Then:
     - `black > 22`: if `x == RIGHT || x == RIGHT_OFF`: `blackColor = topRightIsDark ? tr : (botRightIsDark ? br : blackColor)`; `darkBG = true`; `overallWhite = 0`; `break outer`.
     - `blackStreak`: `darkBG = true`; if `x == RIGHT || x == RIGHT_OFF`: same blackColor rule; if `black > 18`: `overallWhite = 0; break outer`.
     - `whiteStreak || white > 22`: `darkBG = false`.

   The `continue@inner` for a dark pixel pair skips the top-black-streak check and the `blackRun = 0` reset that run after every other pixel. A dark pixel pair therefore extends `blackRun` across the row step. The Rust port keeps this exact control flow; T-44a-081 pins outputs on three fixtures.

8. `topIsBlackStreak = topBlackStreak > topWhiteStreak`; `bottomIsBlackStreak = botBlackStreak > botWhiteStreak`.
9. `if overallWhite > 9 && overallWhite > overallBlack: darkBG = false`.
10. `if topIsBlackStreak && bottomIsBlackStreak: darkBG = true`.
11. If `landscape`: return `Solid(darkBG ? blackColor : WHITE)`.
12. Portrait:
    - `botCornersWhite = is_white(bl) && is_white(br)`; `topCornersWhite = is_white(tl) && is_white(tr)`.
    - `topCornersDark = dTL && dTR`; `botCornersDark = dBL && dBR`.
    - `topOffDark = is_dark(img[LEFT_OFF, TOP]) && is_dark(img[RIGHT_OFF, TOP])`; `botOffDark = is_dark(img[LEFT_OFF, BOT]) && is_dark(img[RIGHT_OFF, BOT])`.
    - match:
      - `darkBG && botCornersWhite` → `Gradient2 { top: blackColor, bottom: WHITE }` (gradient array `[black, black, white, white]` in top-to-bottom order)
      - `darkBG && topCornersWhite` → `Gradient2 { top: WHITE, bottom: blackColor }`
      - `darkBG` → `Solid(blackColor)`
      - `topIsBlackStreak || (topCornersDark && topOffDark && (dTM || overallBlack > 9))` → `Gradient2 { top: blackColor, bottom: WHITE }`
      - `bottomIsBlackStreak || (botCornersDark && botOffDark && (is_dark(bm) || overallBlack > 9))` → `Gradient2 { top: WHITE, bottom: blackColor }`
      - else → `Solid(WHITE)`

Output mapping: `Solid(c)` → solid background; `Gradient2` → `GradientDrawable(TOP_BOTTOM, [a, a, b, b])` with the 4-colour array reduced to two stops as above (the source uses the 4-element array with the first two equal, so a two-stop vertical gradient at 0 and 1 is equivalent: `top` at 0.0, `bottom` at 1.0, with the midpoint held at the first stop until 0.5 and the second from 0.5... Keep the four-stop form `[a, a, b, b]` at offsets `[0, 1/3, 2/3, 1]` (GradientDrawable with 4 colours distributes evenly). Proposal: use four stops at `[0, 1/3, 2/3, 1]` in the GSK gradient node.

Where it is used: pager only, theme 3, static pages (B§8.2). The background is applied in `onImageLoaded`, so the page shows the reader background until ready.

---

## 6. Border crop: `findBorders` (port-ready)

Source: `library/src/main/cpp/borders.cpp:1-215`; constants `borders.h:10-18`. Input: single-channel 8-bit plane (`uint8_t* pixels`, row-major, stride = width). The decoder converts to grayscale before calling it (`borders.h:20-21`). The luminance formula used by the decoder is not in the scope files; the Rust port uses Rec.601 integer luma as a documented choice (verified by T-44a-081 against decoder output on fixtures).

Constants:
- `filledRatioLimit = 0.0025`
- `thresholdForBlack = (uint8_t)(255.0 * 0.75) = 191`
- `thresholdForWhite = (uint8_t)(255.0 - 255.0 * 0.75) = 63`
- `isBlackPixel(p) = p < 191`; `isWhitePixel(p) = p > 63`
- sampling stride 2 along the scan line

`findBorderTop(p, w, h)` (`borders.cpp:117-159`):
```
filledLimit = round(w * 0.0025 / 2) as u32
first row y=0, x in 0, 2, 4, ... < w: black += isBlack; white += isWhite (else-if: a pixel counts at most once)
if white > filledLimit && black > filledLimit: return 0           // mixed fill
detect = (black > filledLimit) ? isWhite : isBlack
for y in 1 .. h:                                                   // ascending
    count = #x in 0,2,4,... < w with detect(p[y*w+x])
    if count > filledLimit: return y
return 0
```

`findBorderBottom(p, w, h)` (`borders.cpp:163-206`): same with `filledLimit = round(w * 0.0025 / 2)`, first row `lastY = h - 1`; scan `y` from `h - 2` down to `1` (descending); return `y + 1` on hit; on mixed fill return `h`; none found → `h`.

`findBorderLeft(p, w, h, top, bottom)` (`borders.cpp:22-65`):
```
filledLimit = round(h * 0.0025 / 2) as u32       // uses full height h
first column x=0, y in top, top+2, ... < bottom: black/white counts (same rule)
if white > filledLimit && black > filledLimit: return 0
detect = (black > filledLimit) ? isWhite : isBlack
for x in 1 .. w:                                  // ascending
    count = #y in top, top+2, ... < bottom with detect(p[y*w+x])
    if count > filledLimit: return x
return 0
```

`findBorderRight(p, w, h, top, bottom)` (`borders.cpp:69-113`): `filledLimit` from `h`; first column `lastX = w - 1`; scan `x` from `w - 2` down to `1`; return `x + 1`; mixed fill or none → `w`.

`findBorders` (`borders.cpp:208-215`):
```
top    = findBorderTop(p, w, h)
bottom = findBorderBottom(p, w, h)
left   = findBorderLeft(p, w, h, top, bottom)
right  = findBorderRight(p, w, h, top, bottom)
rect   = { x: left, y: top, width: right - left, height: bottom - top }
```

Note: the loops in the source use `uint32_t` for limits and `int` loop indices. `round()` is C `round` (ties away from zero), so use `f64::round` then cast. `height * filledRatioLimit / 2` is computed as `double` (`filledRatioLimit` is `const float`, so the product is float-precision; use `f32` for that expression to match).

Decoder integration: the crop rectangle is applied to the decoded image before returning (`cropBorders` flag, `TachiyomiImageDecoder.kt:24` passes it to `ImageDecoder.newInstance`). Rust: compute on the full-resolution grayscale (or on a 1/4 downscale for speed with a documented change; default full resolution to match the observed rectangles).

Rust API: `pub fn find_borders(gray: &[u8], w: u32, h: u32) -> Rect` and `pub fn crop(img: &RgbaImage, r: Rect) -> RgbaImage`. Rect with zero width or height after crop: return the original (matches the implicit behaviour when the mixed-fill and no-fill rules return full extents; if `right - left == 0` the decoder would fail; Rust returns the uncropped image and logs at `warn`).

---

## 7. Animated images

Only GIF and animated WebP are animated (B§9.5). Frame playback:
- Decode all frames up front on the worker (`image::AnimationDecoder::into_frames`), capped at 500 frames or 64 MiB, whichever is hit first. Beyond the cap, keep the first frames and loop them (documented Proposal; the Kotlin/Coil path has no cap but holds the decoder stream).
- Frame delay: GIF delay as given, clamped to `>= 10 ms` (B§14 pitfall 14).
- Playback: the `PageImage` widget schedules a `timeout_add_local_once` for the next frame. No busy loop.
- Loop count: the file's loop count (GIF NETSCAPE extension, WebP `ANIM` loop count); 0 means infinite.
- Pager animated pages are zoomable with PhotoView levels (1, 2, 5) (B§10.6): double tap toggles 1 and 2, pinch clamps to `[1, 5]`.
- Webtoon animated pages are not zoomable (`AppCompatImageView`, B§8.4).

---

## 8. Open items for this topic

1. Pyramid versus per-tile region decode (§4.3): the pyramid is the portable baseline. Decide after measuring the largest real page in the user's library.
2. Border crop on full resolution versus a downscale (§6). Full resolution is the default; a downscale is an optimisation with a visible change in crop rectangles on edge-case pages. Decide with T-44a-081 fixtures.
3. Background gradient stop placement (§5, output mapping). Four stops at 0, 1/3, 2/3, 1 match a `GradientDrawable` with four colours; confirm visually against a Kotlin screenshot of the same page.
