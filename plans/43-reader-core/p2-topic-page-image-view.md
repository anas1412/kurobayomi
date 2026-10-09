# Topic: page image view, zoom and decode (43-reader-core part 2)

Scope file: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt` (442 LOC). Supporting fork source read for exact semantics: tachiyomiorg `subsampling-scale-image-view` at commit `66e0db195d`, file `library/src/main/java/com/davemorrissey/labs/subscaleview/SubsamplingScaleImageView.java` (fetched read-only via `gh api`). Coil and PhotoView are library dependencies (`gradle/libs.versions.toml`: coil 3.1.0, photoview 2.3.0).

## 1. What the class is

A `FrameLayout` that hosts exactly one child view at a time:
- `SubsamplingScaleImageView` (SSIV, non-animated, tiled, zoomable), or `WebtoonSubsamplingImageView` (SSIV whose touch events are ignored; used in the long-strip viewers that own gestures);
- `PhotoView` (animated frames, zoomable), or `AppCompatImageView` (animated frames, not zoomable; webtoon).

Decision per load: `Drawable is Animatable` or `BufferedSource` with `isAnimated = true` → animated child; otherwise SSIV child. The child is replaced only when the kind changes (`prepareNonAnimatedImageView` returns early when the child is already SSIV, lines 247-248; `prepareAnimatedImageView` returns early when the child is an `AppCompatImageView`, line 353, which includes PhotoView).

## 2. Load paths

| Input | Non-animated | Animated |
|---|---|---|
| `Drawable` (BitmapDrawable) | `ImageSource.bitmap(bitmap)` | Coil request with the drawable as data |
| `BufferedSource`, not webtoon or `alwaysDecodeLongStripWithSSIV` | `setHardwareConfig(canUseHardwareBitmap(src))` + `ImageSource.inputStream(src.inputStream())` | Coil request |
| `BufferedSource`, webtoon, forced off | Coil request: `customDecoder(true)`, `precision(INEXACT)`, `size(ViewSizeResolver(this))`, `cropBorders(...)`, memory and disk caches DISABLED; `onSuccess` casts to `BitmapImage` → `ImageSource.bitmap(image.bitmap)` | Coil request |
| any other Drawable (non-bitmap, non-animated) | throws `IllegalArgumentException("Not implemented for class …")` | — |

Observed defects: the non-bitmap throw (no fallback), and the unchecked `result as BitmapImage` cast.

The hardware decision: `ImageUtil.canUseHardwareBitmap(source)` peeks the header (`ImageUtil.kt:594-598`, `extractImageOptions` uses `imageSource.peek()`, so the stream is not consumed), then returns `max(outWidth, outHeight) <= hardwareBitmapThreshold` (`ImageUtil.kt:330-333`). The threshold defaults to `GLUtil.SAFE_TEXTURE_LIMIT` = 2048 (`GLUtil.kt:50`) and is replaced at app start from `pref_hardware_bitmap_threshold` (`App.kt:146`). The result selects SSIV's hardware-config flag. Rust has no equivalent flag: GPU upload is always through `gdk::Texture`; the threshold becomes the tiling decision only (topic §6).

## 3. SSIV configuration (non-animated child)

Set once per child in `prepareNonAnimatedImageView` (lines 256-270):
- `setMaxTileSize(ImageUtil.hardwareBitmapThreshold)`: maximum tile edge.
- `setDoubleTapZoomStyle(ZOOM_FOCUS_CENTER)` (value 2).
- `setPanLimit(PAN_LIMIT_INSIDE)` (value 1).
- `setMinimumTileDpi(180)`.
- `OnStateChangedListener`: `onScaleChanged(newScale, origin)` → `onScaleChanged(newScale)`; `onCenterChanged` ignored.
- `setOnClickListener` → `onViewClicked()`.

Set per load in `setNonAnimatedImage` (lines 293-296): `setDoubleTapZoomDuration(zoomDuration * systemFactor)`, `setMinimumScaleType(minimumScaleType)`, `setMinimumDpi(1)`, `setCropBorders(cropBorders)`.

Fork semantics (verified in `SubsamplingScaleImageView.java`):
- `setMinimumScaleType(int)` throws `IllegalArgumentException("Invalid scale type: …")` unless the value is in `VALID_SCALE_TYPES` = {CENTER_CROP 2, CENTER_INSIDE 1, CUSTOM 7, FIT_WIDTH 3, FIT_HEIGHT 4, ORIGINAL_SIZE 5, SMART_FIT 6} (`SubsamplingScaleImageView.java:137` and `:2072-2075`). The fork's value 4 is FIT_HEIGHT here, not a START constant.
- `setCropBorders(boolean)`: "Set border crop of non-filled (white or black) content" (fork KDoc at line 1537-1541).
- `minScale()` (lines 1904-1931), the fit rule the zoom model reproduces:

| Scale type | minScale |
|---|---|
| CENTER_INSIDE and default | `min(viewW/srcW, viewH/srcH)` |
| CENTER_CROP | `max(viewW/srcW, viewH/srcH)` |
| FIT_WIDTH | `viewW/srcW` |
| FIT_HEIGHT | `viewH/srcH` |
| ORIGINAL_SIZE | `1` |
| SMART_FIT | if `srcH > srcW` → `viewW/srcW`, else `viewH/srcH` |
| CUSTOM | the custom `minScale` field |

Padding terms (`getPaddingLeft/Right/Top/Bottom` and, under PAN_LIMIT_INSIDE, the extra vertical and horizontal space) are subtracted from the view size. The Rust port keeps view padding at 0 (the reader widget has no padding), which is equivalent to the Kotlin when `extra` space is zero.

`limitedScale(target) = max(minScale(), target)` then clamped to `maxScale` (fork). Kotlin sets `maxScale = scale * 5` in `setupZoom` (line 278).

## 4. Zoom start and landscape auto-zoom

`setupZoom(config)` on ready (lines 276-287): `maxScale = scale * 5`; `setDoubleTapZoomScale(scale * 2)`; zoom-start anchor per `ZoomStartPosition` (LEFT → `PointF(0,0)`, RIGHT → `PointF(sWidth, 0)`, CENTER → `center`; null config → nothing).

`landscapeZoom(forward)` (lines 125-161): five preconditions (config present, `landscapeZoom`, `minimumScaleType == CENTER_INSIDE`, `sWidth > sHeight`, `scale == minScale`), then after 500 ms: anchor by `zoomStartPosition` and `forward` (LEFT/forward → (0,0); LEFT/backward → (sWidth,0); RIGHT/forward → (sWidth,0); RIGHT/backward → (0,0); CENTER → center), target scale `height / sHeight`, `animateScaleAndCenter(...).withDuration(500).withEasing(EASE_IN_OUT_QUAD).withInterruptible(true)`.

`onPageSelected(forward)` (lines 102-123): if the page is ready, runs `landscapeZoom(forward)` immediately; otherwise replaces the image-event listener so that on ready it runs `setupZoom`, `landscapeZoom(forward)` and `onImageLoaded`.

## 5. Animated child (PhotoView)

- `setScaleLevels(1F, 2F, MAX_ZOOM_SCALE = 5F)`.
- Double tap: `scale > 1F → setScale(1F, e.x, e.y, true)`, else `setScale(2F, e.x, e.y, true)`.
- Single tap confirmed: `onViewClicked()`, then `super.onSingleTapConfirmed` (returns false).
- `setZoomTransitionDuration(zoomDuration * systemFactor)`.
- Frames: Coil request with the source or drawable, memory and disk caches DISABLED, `crossfade(false)`; on success `setImageDrawable`, `start()` if `Animatable`, visible, `onImageLoaded()`.
- Webtoon animated child: `AppCompatImageView` with `adjustViewBounds = true`, no zoom listeners.

## 6. Decode and tiling (Rust decision)

- Kotlin SSIV tiles large bitmaps by `maxTileSize` (2048 by default) and `minimumTileDpi` 180. Tiles are decoded regionally by `BitmapRegionDecoder` (fork). The Rust port has no region decoder for every format: P0 decodes full RGBA and uploads one texture (downscaled only above a GPU safety cap). Tiling is P1 (T-43b-021): tiles of `hardwareBitmapThreshold` px, decoded from the full RGBA buffer and uploaded lazily per visible region.
- Rule `needs_tiling(w, h, threshold) = max(w, h) > threshold` (default threshold 2048).
- Crop borders (`crop_borders` for paged, `crop_borders_webtoon` for webtoon): the Kotlin fork crops non-filled borders at display time. Rust crops at decode time (`page_decode.rs`), with tolerance defined in T-43b-020.

## 7. Callbacks and lifecycle

- `onImageLoaded` → user lambda, then `background = pageBackground`. Background is the viewer's computed page color (automatic reader theme) and is set only after a load.
- `onImageLoadError` → user lambda (the viewer shows the error and retry).
- `onScaleChanged(scale)` → user lambda (used by the viewer to know zoom state, e.g. to decide page-turn gestures).
- `recycle()`: SSIV `recycle()` or Coil `dispose()` for AppCompatImageView/PhotoView; `isVisible = false`. The child is kept (reused for the next page of the same kind).

## 8. Constants summary

| Name | Value | Where |
|---|---|---|
| MAX_ZOOM_SCALE | 5F | ReaderPageImageView.kt:442 |
| double-tap zoom factor (SSIV) | 2 × scale | :279 |
| landscape delay | 500 ms | :132 |
| landscape duration | 500 ms | :155 |
| pan duration | 250 ms | :241 |
| min DPI on load | 1 | :295 |
| min tile DPI | 180 | :259 |
| zoom style | ZOOM_FOCUS_CENTER (2) | :257 |
| pan limit | PAN_LIMIT_INSIDE (1) | :258 |
| hardware threshold default | 2048 | GLUtil.kt:50 |
| zoom start default (Config) | CENTER | :431 |
| min scale type default (Config) | CENTER_INSIDE (1) | :429 |

## 9. Rust mapping

See p2-rust-design.md §3.8 (widget API), §3.9 (zoom math), §6 (gestures), §7 (decode). Tests: E-01..E-07 and E-12..E-15 in p2-edge-cases-and-tests.md.
