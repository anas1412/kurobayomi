# 44-reader-viewers: Rust design

Target: Kurobayomi (`/home/blackbox/kurobayomi`), Rust 2024 edition, `gtk = { package = "gtk4", version = "0.11", features = ["v4_16"] }` (from `Cargo.toml`), vanilla GTK4 (no libadwaita). This document designs the manga viewer engine. Reader chrome, loaders, view model and settings screens are area:43-reader-core and are referenced, not designed here.

Every Android/Kotlin construct in the scope is mapped in §11. Every observed behaviour that this design must reproduce is referenced by section of p1-behavior.md (B§n) or by source path.

---

## 1. Principles

1. **Pure logic first.** Everything that decides *what* to show (item lists, tap zones, preload windows, scale types, zoom clamping, dual-page placement, border cropping, background choice) is a pure Rust function on plain data. It is unit-testable without GTK. This is where the observed behaviour lives (B§2, §5, §6.3, §9, §10.2, §10.7).
2. **Widgets only draw and forward input.** GTK widgets own rendering and gesture capture. They call into the controllers via callbacks and never decide navigation themselves.
3. **GTK main thread owns every GObject.** `gdk::Texture`, widgets and `gtk::Snapshot` are not `Send`. Worker threads produce plain RGBA buffers; textures are created on the main thread.
4. **No viewer-owned persistence.** Decoded pages are an in-memory LRU. Progress is reported upward (area:43).
5. **Preserve observed behaviour unless a proposal is explicitly labelled.** The few deviations are listed in p1-behavior.md §15 and in the tasks.

---

## 2. Crate and module layout

All paths relative to `/home/blackbox/kurobayomi`. New code lives under `src/reader/`. The existing mock `src/player.rs` `reader()` is replaced by `reader::ReaderView` (§12).

```
src/reader/
  mod.rs                       // pub use; ReaderView (GTK entry), make_viewer()
  viewer.rs                    // trait ViewerEngine, ViewerEvent, ViewerKind, ViewerChapters
  settings.rs                  // ViewerSettings, ViewerSettingsWatch, defaults, key names (§9)
  model/
    mod.rs
    page.rs                    // PageRef, PageKind { Normal, Insert { parent_index } }, PageStatus
    chapter.rs                 // ChapterHandle (Arc), ChapterState (watch), ChapterTransitionKind
    items.rs                   // ListItem enum, ItemKey, ChapterTransition (Prev/Next, unordered-eq)
    gap.rs                     // calculate_chapter_gap (B§5.2)
  nav/
    mod.rs                     // Region, NavRegion, NavAction, hit_test, invert
    layouts.rs                 // six const tables (B§2.3)
    colors.rs                  // ARGB constants (B§2.5)
  input/
    mod.rs                     // InputAction enum, KeyMap, ScrollMap
    keys.rs                    // pager and webtoon key tables (B§4.2, §4.4)
    gestures.rs                // GTK gesture wiring -> InputAction (B§3, §3.4)
  pager/
    mod.rs                     // PagerController (B§6)
    items.rs                   // build_pager_items (B§5.3), preprocessed map, R2L reverse
    nav.rs                     // next_index / prev_index / pan decision (B§6.3)
    preload.rs                 // preload window and checkAllowPreload (B§6.4)
  webtoon/
    mod.rs                     // WebtoonController (B§7)
    items.rs                   // build_webtoon_items (B§5.5)
    scroll.rs                  // scroll distance, current item, menu threshold (B§7.2-7.4, §7.6)
  split.rs                     // dual-page side choice and InsertPage placement (B§9.1, §5.6)
  decode/
    mod.rs                     // DecodeRequest, DecodedPage, decode_worker()
    formats.rs                 // format sniffing, animated detection (B§9.5)
    borders.rs                 // find_borders() exact port (B§9.4)
    background.rs             // choose_background() exact port (B§9.3)
    transform.rs               // split_in_half, split_and_merge, rotate (B§9.1-9.2)
    lru.rs                     // byte-budgeted LRU of decoded pages (§6)
  zoom/
    mod.rs                     // ZoomState, pure math shared by pager and webtoon
    scale.rs                   // ScaleType enum, min_scale(), limited_scale() (B§10.2)
    pinch.rs                   // pinch and quick-scale update rules (B§10.4)
    pan.rs                     // clamp rules, pan_remaining() (B§10.4, §10.7)
    tween.rs                   // Tween, Ease (Decelerate, OutQuad, InOutQuad), frame-clock driver
  widgets/
    mod.rs
    page_image.rs              // PageImage: GtkWidget, one static or animated page (B§10.1, §10.6)
    pager_view.rs              // PagerView: GtkWidget, paged strip (B§6.1, §6.6)
    webtoon_view.rs            // WebtoonView: GtkWidget, virtual vertical list + zoom (B§7, §10.7)
    transition.rs              // TransitionCard: GtkWidget (B§11)
    status.rs                  // PageStatusOverlay: spinner, determinate arc, error card (B§8.5, §8.6)
    overlay_colors.rs          // not separate: colours come from nav::colors
```

Test layout: `src/reader/**/tests.rs` (unit), `tests/reader_golden.rs` (fixtures, §10 of p1-edge-cases-and-tests.md).

---

## 3. Core types and trait signatures

### 3.1 Identifiers and chapter state

```rust
// src/reader/model/chapter.rs
use std::sync::Arc;
use tokio::sync::watch;

pub type ChapterId = i64;              // domain chapter id (area:05/area:07)

#[derive(Clone, Debug, PartialEq)]
pub enum ChapterLoadState {            // B§5.1 ReaderChapter.State
    Wait,
    Loading,
    Error(Arc<str>),                    // Throwable -> message string (B§11.3 uses message)
    Loaded(Arc<[PageRef]>),
}

/// Shared handle for one chapter. Replaces ReaderChapter's StateFlow with a watch channel.
#[derive(Clone)]
pub struct ChapterHandle {
    pub id: ChapterId,
    pub number: f64,                    // chapter_number; used by calculate_chapter_gap (B§5.2)
    pub recognised_number: bool,        // domain isRecognizedNumber
    pub name: Arc<str>,
    pub url: Arc<str>,
    pub scanlator: Option<Arc<str>>,
    pub requested_page: usize,          // B§5.1 requestedPage (0)
    pub state: watch::Receiver<ChapterLoadState>,   // B§5.1 stateFlow
    pub loader: Option<Arc<dyn PageLoader>>,        // B§5.1 pageLoader (area:43 implements)
    pub is_local: bool,                 // pageLoader.isLocal (B§11.2)
}
```

`ChapterHandle` is an `Arc` clone of data owned by area:43's view model. Equality for `ChapterTransition` is by `id` (domain equality of `Chapter` is structural in Kotlin; `id` plus `url` is the Rust equivalent and is stated in p1-edge-cases-and-tests.md E-03).

### 3.2 Pages and items

```rust
// src/reader/model/page.rs
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PageKind { Normal, Insert { parent_index: usize } }   // B§5.1 InsertPage

#[derive(Clone)]
pub struct PageRef {
    pub chapter: ChapterId,             // B§5.1 chapter (identity by id)
    pub index: usize,                   // B§5.1 index (0-based)
    pub kind: PageKind,
    pub url: Arc<str>,
    pub image_url: Option<Arc<str>>,    // B§5.1 imageUrl
    pub status: watch::Receiver<PageStatus>,    // Page.statusFlow
    pub progress: watch::Receiver<u8>,          // Page.progressFlow, 0..=100
    pub stream: Option<Arc<dyn Fn() -> Result<Box<dyn std::io::Read + Send>, PageError> + Send + Sync>>,
}

impl PageRef {
    pub fn number(&self) -> usize { self.index + 1 }       // Page.number (B§5.1, Page)
    pub fn key(&self) -> PageKey {                          // identity for indexOf / DiffUtil
        PageKey { chapter: self.chapter, index: self.index, insert: matches!(self.kind, PageKind::Insert { .. }) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PageKey { pub chapter: ChapterId, pub index: usize, pub insert: bool }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageStatus { Queue, LoadPage, DownloadImage, Ready, Error }  // Page.State order (B§8.1)
```

```rust
// src/reader/model/items.rs
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TransitionKind { Prev, Next }

/// Equality is the unordered pair rule of B§5.1: Prev(A,B) == Next(B,A).
#[derive(Clone)]
pub struct ChapterTransition {
    pub kind: TransitionKind,
    pub from: ChapterHandle,            // ChapterTransition.from
    pub to: Option<ChapterHandle>,      // ChapterTransition.to
}
fn pair(t: &ChapterTransition) -> (ChapterId, Option<ChapterId>) {
    (t.from.id, t.to.as_ref().map(|c| c.id))
}

impl PartialEq for ChapterTransition {
    // Mirrors ChapterTransition.equals (B§5.1): same pair, or from == other.to && to == other.from.
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (pair(self), pair(other));
        a == b || (b.1 == Some(a.0) && a.1 == Some(b.0))
    }
}
```

Note on `PartialEq`: `b.1 == Some(a.0)` is `from == other.to`, and `a.1 == Some(b.0)` is `to == other.from`. A `None` `to` never matches, as in Kotlin where `null == x` is false for a non-null `x`. The test in E-03 pins this.

```rust
#[derive(Clone)]
pub enum ListItem {
    Page(PageRef),                      // ReaderPage or InsertPage
    Transition(ChapterTransition),      // ChapterTransition.Prev / .Next
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKey { Page(PageKey), Transition { kind: TransitionKind, from: ChapterId, to: Option<ChapterId> } }
```

`ItemKey` is the diff key used by the webtoon list (replaces `DiffUtil` `areItemsTheSame`, B§5.5). `areContentsTheSame` is always true (B§5.5), so the webtoon view rebinds only items whose key is new.

### 3.3 Viewer contract

```rust
// src/reader/viewer.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerKind { LeftToRight, RightToLeft, Vertical, Webtoon, WebtoonGapped }  // B§0

pub struct ViewerChapters {                         // B§5.1
    pub curr: ChapterHandle,
    pub prev: Option<ChapterHandle>,
    pub next: Option<ChapterHandle>,
}

/// Events a viewer sends up to ReaderView (area:43 view model).
pub enum ViewerEvent {
    PageSelected(PageRef),                          // -> activity.onPageSelected (B§13)
    PageLongTap(PageRef),                           // -> open page actions (B§3.2)
    PreloadChapter(ChapterHandle),                  // -> requestPreloadChapter (B§12.1)
    ToggleMenu,
    ShowMenu,                                       // only when hidden (B§13)
    HideMenu,                                       // only when visible (B§13)
    SplitPage { parent: PageRef },                  // dual-page split request (B§5.6 onPageSplit)
}

pub trait ViewerEngine {
    fn kind(&self) -> ViewerKind;
    fn widget(&self) -> &gtk::Widget;               // B§6.1 getView
    fn set_chapters(&self, chapters: ViewerChapters);      // B§6.2 / B§7.5
    fn move_to_page(&self, key: PageKey);           // B§6.6 / B§7.5
    fn handle_key(&self, key: &InputKey) -> bool;   // B§4 handleKeyEvent
    fn handle_scroll(&self, dy: f64) -> bool;       // B§4.3 handleGenericMotionEvent
    fn destroy(&self);                              // B§6.8 / B§7.8
}
```

`ViewerEngine` is not `Send`. It is held by `ReaderView` on the GTK thread, as a `Box<dyn ViewerEngine>` in an `Rc<RefCell<_>>`. `ViewerEvent`s travel to area:43 over a `glib` channel (§4).

`InputKey` is a small enum produced from `gdk::Key` plus modifier state (§8).

### 3.4 Settings

```rust
// src/reader/settings.rs
pub struct ViewerSettings {                         // B§1.1 (config), B§1.3 (enums), B§1.9 (values)
    pub long_tap_enabled: bool,                     // default true
    pub use_page_transitions: bool,                 // default true (pref)
    pub double_tap_anim_ms: u32,                    // default 500
    pub volume_keys_enabled: bool,                  // default false
    pub volume_keys_inverted: bool,                 // default false
    pub always_show_chapter_transition: bool,       // default true
    pub navigation_overlay_on_start: bool,          // default false
    pub nav_inverted_pager: TappingInvert,          // default None
    pub nav_inverted_webtoon: TappingInvert,        // default None
    pub nav_mode_pager: NavMode,                    // default Default(0)
    pub nav_mode_webtoon: NavMode,                  // default Default(0)
    pub scale_type: ScaleTypePref,                  // default FitScreen(1)
    pub zoom_start: ZoomStartPref,                  // default Automatic(1)
    pub theme: ReaderTheme,                         // default Black(1)
    pub crop_borders_pager: bool,                   // default false
    pub crop_borders_webtoon: bool,                 // default false
    pub navigate_pan: bool,                         // default true
    pub landscape_zoom: bool,                       // default true
    pub webtoon_side_padding: u8,                   // default 0, 0..=25
    pub hide_threshold: HideThreshold,              // default Low (31)
    pub webtoon_zoom_out_disabled: bool,            // default false
    pub webtoon_double_tap_zoom: bool,              // default true
    pub dual_split_pager: bool,                     // default false
    pub dual_invert_pager: bool,                    // default false
    pub dual_rotate_pager: bool,                    // default false
    pub dual_rotate_invert_pager: bool,             // default false
    pub dual_split_webtoon: bool,                   // default false
    pub dual_invert_webtoon: bool,                  // default false
    pub dual_rotate_webtoon: bool,                  // default false
    pub dual_rotate_invert_webtoon: bool,           // default false
    pub max_tile_px: u32,                           // default 2048 (pref_hardware_bitmap_threshold)
    pub always_decode_strip_with_ssiv: bool,        // default false (P3 drop, T-44a-061)
}
```

Each field is a `tokio::sync::watch::Sender<T>`-backed value in the settings store (§9). Enum types (`TappingInvert`, `NavMode`, `ScaleTypePref`, `ZoomStartPref`, `ReaderTheme`, `HideThreshold`) carry the stored integer or name, and their `from_stored` constructors fall back to the default for unknown values, matching `when (value) { ... else -> default }` in the Kotlin (B§1.3).

---

## 4. Threading and async model

| Work | Thread | Mechanism | Observed in |
|---|---|---|---|
| GTK widgets, controllers, gestures, tweens | GTK main thread | `glib::MainContext`; `gtk::glib::spawn_future_local` | B§12.1 |
| Page byte load (loader) | Tokio runtime | `tokio::spawn` (area:43 loaders) | `launchIO` B§8.1 |
| Decode and transforms (split, rotate, borders, background) | Tokio blocking pool (`spawn_blocking`) or `rayon` for tile work | `DecodeRequest` → `DecodedPage` | `withIOContext` B§8.2 |
| Status and progress observation | Main thread | `watch::Receiver::changed()` inside `spawn_future_local` | `statusFlow.collectLatest` B§8.1 |
| Preload request | Tokio | `ViewerEvent::PreloadChapter` → area:43 `preload` | `requestPreloadChapter` B§12.1 |
| Frame-clock animations | GTK main thread | `gtk::Widget::add_tick_callback` | `ValueAnimator` B§10.7 |

Bridge: decoded results go back to the GTK thread via `async_channel::Sender<DecodeResult>` polled by `glib::spawn_future_local`. Texture creation happens there.

```rust
// src/reader/decode/mod.rs
pub struct DecodeRequest {
    pub key: PageKey,
    pub bytes: Arc<[u8]>,                       // read by the loader; owned
    pub intent: DecodeIntent,
}
pub struct DecodeIntent {
    pub transform: PageTransform,               // B§9.1/9.2 decision, computed on main thread
    pub crop_borders: bool,                     // B§9.4
    pub auto_background: bool,                  // B§9.3 (pager, theme 3, static only)
    pub target_width: Option<u32>,              // webtoon: view width * scale factor (B§8.4); None = full
    pub landscape: bool,                        // chooseBackground landscape branch (B§9.3 step 11)
}
pub enum PageTransform { None, RotateWide { invert: bool }, SplitLeft, SplitRight, SplitAndMerge { upper_right: bool } }

pub enum DecodedPage {
    Still { rgba: Arc<RgbaBuffer>, background: Option<Background> },
    Animated { frames: Arc<[AnimFrame]>, loop_count: u16 },
}
pub struct RgbaBuffer { pub width: u32, pub height: u32, pub stride: usize, pub data: Vec<u8> }
pub enum Background { Solid(Rgba), Gradient2 { top: Rgba, bottom: Rgba } }   // B§9.3 steps 11-12
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);
pub struct AnimFrame { pub buffer: RgbaBuffer, pub delay_ms: u32 }
pub enum DecodeError { Unsupported(String), Corrupt(String), Io(String) }
```

Cancellation: each holder/`PageRef` owns a `tokio_util::sync::CancellationToken`. Widget unrealise or `unbind` cancels it (replaces `loadJob.cancel()`, B§8.1, §12.3). A decode that finishes after cancel is dropped.

Ordering: decode results for a page are applied only if the page is still bound (`ItemKey` still present and the generation counter matches). This matches `collectLatest` semantics (B§12.3).

---

## 5. Decode pipeline

### 5.1 Format sniffing

`formats.rs`: `image::guess_format` on the first bytes gives the coarse type (`Png`, `Jpeg`, `Gif`, `WebP`, `Avif`, ...). Animated detection (B§9.5):
- GIF → animated (decode all frames with `image::AnimationDecoder` on `GifDecoder`).
- WebP → parse RIFF; `ANIM` chunk present → animated (decode frames with the `image` crate WebP decoder's animation support; if unavailable in the pinned version, use `libwebp` through the `webp` crate's `AnimDecoder`).
- AVIF, HEIF, JXL with animation → not animated (first frame, B§9.5 proposal, T-44a-052).

### 5.2 Still decode

- Default: `image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode()?` → `to_rgba8()`.
- JPEG fast path (optional, T-44a-050): `zune-jpeg` with `ColorSpace::RGBA` output. Keep as an option; `image` is the baseline.
- JXL: `jxl-oxide` (pure Rust). HEIF: `libheif-rs` behind a Cargo feature `heif`, off by default.
- Large images: tile into `max_tile_px` (default 2048, B§10.1) squares, one `RgbaBuffer` per tile, so GPU textures never exceed the device limit. The tile grid is computed in `zoom::tiles`.
- Webtoon: decode directly to `target_width` with a Lanczos3 or Triangle downscale when the source is wider than the target (replaces Coil `ViewSizeResolver` + `Precision.INEXACT`, B§8.4). Power-of-two sampling is not required; `fast_image_resize` is the optional crate.

### 5.3 Transforms (exact)

`transform.rs` operates on `image::RgbaImage` and reproduces B§9.1 and B§9.2:
- `is_wide(w, h) -> bool { w > h }` (strict).
- `split_in_half(img, side) -> RgbaImage`: `half_w = w / 2` (integer); `RIGHT` copies columns `[w - half_w, w)`, `LEFT` copies `[0, half_w)`; output `(half_w, h)`.
- `split_and_merge(img, upper_right: bool) -> RgbaImage`: output `(w / 2, 2h)`; the upper cell gets the RIGHT (or LEFT) half, the lower cell the complementary half (B§9.2).
- `rotate(img, degrees: f32)`: ±90 only in this area (`postRotate(90 or -90)`), so use `image::imageops::rotate90`/`rotate270`. The Android `Matrix.postRotate` result has the same pixel layout for exact 90° multiples.

No JPEG re-encoding is needed in Rust: the observed JPEG q100 round trip (B§9.1) is an implementation artefact. Keep pixels lossless (documented as an intentional improvement; the rounding difference is below one code value at q100).

### 5.4 Border cropping

`borders.rs`: exact port of `findBorders` (B§9.4). Input is the luminance plane (`Y = (299 r + 587 g + 114 b) / 1000` as u8; the Android path uses the decoder's grayscale output, so this is a documented choice, verified by the golden test T-44a-081). Constants: `FILLED_RATIO_LIMIT: f64 = 0.0025`, `BLACK_THRESHOLD: u8 = 191`, `WHITE_THRESHOLD: u8 = 63`, `STRIDE: usize = 2`.

Integer details that must match C: `filledLimit = round(length * 0.0025 / 2) as u32` where `round` is C `round` (half away from zero, `f64::round`).

### 5.5 Automatic background

`background.rs`: exact port of `chooseBackground` steps B§9.3 (1-12). Returns `Background::Solid` or `Gradient2`. The landscape branch uses the widget's allocated width > height instead of the device orientation (documented proposal; same visible result in a landscape window).

### 5.6 Memory budget

`lru.rs`: byte-budgeted LRU keyed by `PageKey` (default budget 512 MiB; configurable; the Kotlin keeps only ~2 pages around the current page through the adapter window, B§12.2). Eviction only for pages outside the window (`pager`: current ± 1 plus preload pages; `webtoon`: visible range ± 3 items, matching the `refreshAdapter` ±3 window of B§7.7). Evicted pages revert to status `Queue`, not `Error`.

---

## 6. Pager engine

### 6.1 Widget: `PagerView`

A `gtk::Widget` subclass (glib object subclass, `ObjectImpl` + `WidgetImpl`). Properties: `orientation` (horizontal or vertical), `current: usize`, `items_len: usize`.

Layout: children are laid out in a strip of `(item_width or item_height)` each, the viewport shows the current item. Offset animation: `Tween` on the frame clock (§7) from the current offset to the target index offset. Duration: `use_page_transitions ? 250 ms : 0` (Proposal: 250 ms; the observed Android ViewPager smooth-scroll duration is platform-defined, and 250 ms is the default ViewPager duration. Pin in T-44a-024.)

Offscreen window: only items within `current ± 1` are realised (`offscreenPageLimit = 1`, B§6.1). Other items are unrealised and their widgets dropped (their decode state is kept in the LRU).

Swipe: `gtk::GestureDrag` on the horizontal (or vertical) axis; on release, if `|offset| > 0.5 * extent` or velocity > threshold, snap to next or previous, otherwise snap back. The Android `ViewPager` drag thresholds are platform values; the GTK equivalent is `gtk-dnd-drag-threshold` for the start and a 50% rule for the commit (Proposal).

### 6.2 Controller

```rust
// src/reader/pager/mod.rs
pub struct PagerController {
    kind: ViewerKind,                       // LeftToRight | RightToLeft | Vertical
    settings: Rc<RefCell<ViewerSettings>>,
    items: Vec<ListItem>,                   // B§5.3 output (R2L already reversed)
    preprocessed: BTreeMap<usize, PageRef>, // B§5.3 "preprocessed" (key = parent index)
    current_item: Option<usize>,            // B§6.4 currentPage (index into items)
    current_page: Option<ItemKey>,          // currentPage identity
    next_transition: Option<ChapterTransition>,   // B§5.3 nextTransition (always assigned)
    current_chapter: Option<ChapterId>,     // B§5.3 currentChapter
    awaiting_idle: Option<ViewerChapters>,  // B§6.2 awaitingIdleViewerChapters
    is_idle: bool,                          // B§6.1 isIdle (initially true)
    view: PagerView,
    events: EventSender,
}
```

Key methods (signatures; logic per the cited sections):
```rust
impl PagerController {
    pub fn set_chapters(&mut self, c: ViewerChapters);        // B§6.2: defer if !is_idle
    fn apply_chapters(&mut self, c: ViewerChapters);          // B§6.2 setChaptersInternal
    fn build_items(&mut self, c: &ViewerChapters, force: bool) -> Vec<ListItem>; // B§5.3
    pub fn move_next(&mut self);                              // B§6.3 moveToNext (per kind)
    pub fn move_prev(&mut self);                              // B§6.3 moveToPrevious
    fn move_right(&mut self);                                 // B§6.3 (absolute)
    fn move_left(&mut self);                                  // B§6.3 (absolute)
    pub fn move_to_page(&mut self, key: PageKey);             // B§6.6 (smooth per open question 1)
    fn on_index_changed(&mut self, idx: usize);               // B§6.4 onPageChange
    fn on_reader_page_selected(&mut self, p: &PageRef, allow_preload: bool, forward: bool); // B§6.4
    fn on_transition_selected(&mut self, t: &ChapterTransition);                            // B§6.4
    pub fn on_split(&mut self, parent: &PageRef, insert: PageRef);                          // B§5.6
    pub fn on_set_idle(&mut self, idle: bool);                // B§6.1 isIdle setter
}
```

`move_next` and `move_prev` are the only places R2L differs: `RightToLeft` maps `move_next → move_left` and `move_prev → move_right` (B§6.3). `Vertical` uses the base mapping, where `move_next` is `move_right` (index + 1).

`next_index(idx, len)` and `prev_index(idx, len)` are pure: `None` at the boundary (no wrap, B§6.3 "Edge").

`pan_or_turn(dir)`: if `navigate_pan && holder.can_pan(dir)` → `holder.pan(dir)` (250 ms, B§10.4 pan rule; the pan animation is the `zoom::tween` OutQuad over 250 ms), else `view.set_current(idx ± 1, use_page_transitions)`.

### 6.3 Preload

`preload.rs` (pure):
```rust
pub fn in_preload_range(page_number: usize, chapter_pages: usize) -> bool { chapter_pages - page_number < 5 }  // B§6.4 (note usize: guard page_number <= chapter_pages)
pub fn check_allow_preload(page_chapter: Option<ChapterId>, current: Option<&ListItem>, next_to: Option<ChapterId>) -> bool  // B§6.4
```
`in_preload_range` must guard `page_number <= chapter_pages` (Kotlin uses signed ints; `pages.size - number` is never negative for valid pages, and the Rust version uses saturating subtraction to avoid underflow).

### 6.4 Transition gating

`pager/items.rs::build_pager_items` is the direct port of B§5.3 (12 steps). `force` is computed by the caller as `always_show || items[current] is transition` (B§5.4). The function is pure and takes `&ViewerChapters`, `force`, `preprocessed`, and `kind`.

---

## 7. Webtoon engine

### 7.1 Widget: `WebtoonView`

A `gtk::Widget` implementing `gtk::Scrollable` (`hadjustment`, `vadjustment`, `hscroll-policy`, `vscroll-policy`). It keeps:
- `items: Vec<ListItem>` (B§5.5)
- `heights: Vec<f64>`: per-item height in widget px, computed from `(page_width * decoded_aspect)` once dims are known; placeholder height is `viewport_height` (B§8.6 "at least one viewport height") until the page is decoded. Heights are prefix-summed into `offsets: Vec<f64>` for O(log n) lookup (`partition_point`).
- `extra_layout: f64 = viewport_height * 3 / 4` (B§7.1 `scrollDistance`).
- `realised: Range<usize>`: items with `offset + height` in `[scroll - extra_layout, scroll + viewport + extra_layout]`.

Side padding (B§8.6): horizontal margin = `width * padding_pct / 100` (integer), applied to both sides; gapped mode adds 15 dp (scaled by the GTK scale factor) below each page.

Zoom: the strip is drawn with a transform `translate(x, y) scale(s)` in `snapshot`, not by scaling the widget. The zoom state is `zoom::ZoomState` (§8). Pointer coordinates are converted back into strip coordinates by inverting the transform (replaces the Android `WebtoonFrame` coordinate translation, B§3.4).

Scrolling: the vertical scroll is `gtk::Scrollable`'s `vadjustment`. Keyboard and tap scroll call `vadjustment.set_value(value ± scroll_distance)` with `animate` when `use_page_transitions` (B§7.4).

### 7.2 Controller

```rust
pub struct WebtoonController {
    gapped: bool,                            // ViewerKind::WebtoonGapped (B§0)
    settings: Rc<RefCell<ViewerSettings>>,
    items: Vec<ListItem>,
    current_chapter: Option<ChapterId>,      // B§5.5 currentChapter
    current_item: Option<ItemKey>,           // B§7.3 currentPage
    hide_threshold: u32,                     // read per scroll now (proposal, B§7.6)
    view: WebtoonView,
}
```

Pure helpers (`webtoon/scroll.rs`):
```rust
pub fn scroll_distance(viewport_h: i32) -> i32 { viewport_h * 3 / 4 }          // B§7.1 (integer division)
pub fn last_end_visible(offsets: &[f64], heights: &[f64], viewport: (f64, f64)) -> Option<usize>  // B§7.3
pub fn menu_should_hide(dy: f64, threshold: u32, menu_visible: bool) -> bool   // B§7.2 step 2 (|dy| > threshold)
pub fn preload_on_scroll_up(first: &ListItem) -> Option<ChapterHandle>        // B§7.2 step 3
pub fn end_of_book_shows_menu(last: &ListItem) -> bool                         // B§7.2 step 4
```

`last_end_visible` returns the last item index whose end is `<= viewport_end` or whose start is `< viewport_start`, scanned from the bottom (B§7.3, `findLastEndVisibleItemPosition`). `None` when nothing is realised.

---

## 8. Zoom model

### 8.1 Shared state

```rust
// src/reader/zoom/mod.rs
#[derive(Clone, Copy, Debug)]
pub struct ZoomState {
    pub scale: f64,              // SSIV scale (pager) / currentScale (webtoon)
    pub center: (f64, f64),      // SSIV center in source px (pager) / translate (webtoon, see §8.4)
    pub min_scale: f64,          // SSIV minScale() (pager) / 0.5 or 1.0 (webtoon)
    pub max_scale: f64,          // SSIV maxScale: 5 * fit (pager) / 3.0 (webtoon)
    pub double_tap_scale: f64,   // 2 * fit (pager); 2.0 (webtoon)
}
```

Pure operations (`zoom/scale.rs`, `zoom/pan.rs`):
- `min_scale(kind, view: (f64,f64), src: (f64,f64), pad, custom) -> f64` with the table of B§10.2 (CENTER_INSIDE min of ratios, CENTER_CROP max, FIT_WIDTH, FIT_HEIGHT, ORIGINAL_SIZE=1, SMART_FIT, CUSTOM).
- `limited_scale(t, min, max) = t.max(min).min(max)` (B§10.2).
- `pan_remaining(scale, src, view, translate) -> Rect` for PAN_LIMIT_INSIDE (B§10.4); `can_pan_right = remaining.right > 1.0` (B§10.4).
- `fit_to_bounds(…)` for PAN_LIMIT_INSIDE: when the scaled image is smaller than the view on an axis, centre it on that axis; when larger, clamp translate to `[view - scaled, 0]` (SSIV `fitToBounds` semantics, documented in p1-topic-zoom-and-image.md §3).

### 8.2 Double tap (pager, SSIV rule B§10.5)

```rust
pub fn double_tap_target(scale: f64, min: f64, dt_scale: f64, max: f64) -> f64 {
    let dt = dt_scale.min(max);
    let zoom_in = scale <= dt * 0.9 || scale == min;
    if zoom_in { dt } else { min }
}
```
Animation: `Tween` from `scale` to target, centred on the source point under the touch, duration `double_tap_anim_ms`, not interruptible (B§10.5).

### 8.3 Pinch and quick scale (pager SSIV rules B§10.4)

`pinch.rs`:
```rust
pub fn pinch_scale(dist_now: f64, dist_start: f64, scale_start: f64, max: f64) -> f64 {   // B§10.4
    (max).min((dist_now / dist_start) * scale_start)
}
pub fn quick_scale_step(dy_start: f64, dy_now: f64, last_dist: Option<f64>, threshold_px: f64,
                        scale: f64, min: f64, max: f64, upwards: bool) -> (f64, f64, bool) {
    // dist = |dy_start - dy_now| * 2 + threshold_px
    // span = |1 - dist / last| * 0.5 (last defaults to dist when None)
    // moved if span > 0.03 or already moved
    // multiplier = upwards ? 1 + span : 1 - span; scale = clamp(scale * multiplier, min, max)
    // returns (new_scale, dist, moved)
}
```
`upwards` is computed as `dy_now > dy_last` (B§10.4, exact comparison kept). `threshold_px = 20 dp in px` (B§12.2).

### 8.4 Webtoon zoom (B§10.7)

```rust
pub struct WebtoonZoom { pub scale: f64, pub tx: f64, pub ty: f64, pub half_w: f64, pub half_h: f64, pub orig_h: f64, pub zoom_out_disabled: bool }
impl WebtoonZoom {
    pub fn min_rate(&self) -> f64 { if self.zoom_out_disabled { 1.0 } else { 0.5 } }   // MIN_RATE / DEFAULT_RATE
    pub fn clamp_x(&self, x: f64) -> f64 { if self.scale < 1.0 { 0.0 } else { let m = self.half_w * (self.scale - 1.0); x.clamp(-m, m) } }
    pub fn clamp_y(&self, y: f64) -> f64 { if self.scale < 1.0 { self.orig_h / 2.0 - self.half_h } else { let m = self.half_h * (self.scale - 1.0); y.clamp(-m, m) } }
    pub fn apply_pinch(&mut self, factor: f64) -> f64 /* new layout height */ { /* B§10.7 onScale steps 1-5 */ }
    pub fn double_tap(&mut self, touch_x: f64, touch_y: f64) -> Tween { /* B§10.7 double-tap rule */ }
    pub fn fling(&mut self, vx: f64, vy: f64, at_edge: bool) -> Option<Tween> { /* B§10.8 */ }
}
```
Note: `clamp_y` when `scale < 1` returns the vertical centring value, not a clamp (B§10.7).

### 8.5 Tween and easing

```rust
// zoom/tween.rs
pub enum Ease { Decelerate /* 1-(1-t)^2 */, OutQuad, InOutQuad }
pub struct Tween { pub from: f64, pub to: f64, pub start_us: i64, pub dur_us: i64, pub ease: Ease, pub interruptible: bool }
pub fn ease(e: Ease, t: f64) -> f64 { match e {
    Ease::Decelerate => 1.0 - (1.0 - t) * (1.0 - t),
    Ease::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),          // EASE_OUT_QUAD
    Ease::InOutQuad => if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 },
} }
```
Driven by `widget.add_tick_callback(|w, clock| { ... })` using `clock.frame_time()`. Duration zero or `animator_duration_scale == 0` → one-frame jump (see §11 mapping for `gtk-enable-animations`).

`DecelerateInterpolator` (factor 1) and `EASE_OUT_QUAD` are the same polynomial. The SSIV `EASE_OUT_QUAD` is written as `1 - (1-t)^2` in the Android source; `ease()` documents that both names resolve to it.

---

## 9. Settings store integration

The viewer reads settings through `ViewerSettings`. Source of truth is the Kurobayomi settings store (`src/settings.rs`, the existing mock, and the `spec/settings.json` schema; area:02 owns the schema). Keys are the Aniyomi key strings in p1-data-and-config.md §3 so a future import from Aniyomi backup maps 1:1.

Watch semantics (B§1.1 proposal):
- `ViewerSettings` is an `Rc<SettingsWatch>` holding one `tokio::sync::watch` channel per field.
- Consumers read on construction and then `changed()` (replaces `drop(1)`).
- A change is pushed only when the new value differs (`PartialEq` check before `send_replace`), matching `distinctUntilChanged`.

Re-application (B§1.2):
- image property changes → `refresh_all_pages()` (pager: rebuild holders; webtoon: rebind visible range ±3).
- theme change → restyle in place (no activity recreate; T-44a-043).
- navigation mode / inversion change → recompute `nav::Table` and overlay.

---

## 10. Input wiring

`input/gestures.rs` (widget-level gesture set; see p1-behavior.md §3.4):

| GTK controller | Attached to | Emits |
|---|---|---|
| `GestureClick` (n_press = 1, on release) | viewer widget | `InputAction::Tap { x_norm, y_norm }` (pager: immediately; webtoon: after double-click window) |
| `GestureClick` (n_press = 2) | page image / webtoon | `InputAction::DoubleTap { x, y }` |
| `GestureLongPress` | viewer widget | `InputAction::LongPress { x, y }` (gated by `long_tap_enabled || menu_visible`) |
| `GestureZoom` | page image / webtoon | `InputAction::Pinch { factor }` |
| `GestureDrag` | pager (swipe), webtoon (pan when zoomed), page image (pan) | `InputAction::Drag { dx, dy, phase }` |
| `GestureSwipe` | webtoon | `InputAction::Fling { vx, vy }` (zoomed only) |
| `EventControllerScroll` (VERTICAL) | viewer widget | `InputAction::Scroll { dy }` (pager: §4.3; webtoon: native scroll, not consumed) |
| `EventControllerKey` (capture phase) | window | `InputAction::Key(InputKey)` (viewer first, then window; §4.1) |

Tap coordinates: normalised by the widget's allocated size. Webtoon uses the widget's *original* height (captured at first allocation), not its current height, to match B§2.1.

`InputKey` (from `gdk::Key` + `gdk::ModifierType`):
```rust
pub enum InputKey { Left, Right, Up, Down, PageUp, PageDown, Menu, VolumeUp, VolumeDown, CtrlRight, CtrlLeft, Other }
```
Mapping: `gdk::Key::Left` → `Left`, `Right`, `Up`, `Down`, `Page_Up`, `Page_Down`, `Menu`, `XF86AudioRaiseVolume` → `VolumeUp`, `XF86AudioLowerVolume` → `VolumeDown`. Ctrl is `ModifierType::CONTROL_MASK`. Key release is `EventControllerKey::connect_key_released` (ACTION_UP equivalent); key press is the down equivalent. The Kotlin returns `true` on both down and up for listed keys (B§4.2), so the Rust controller must claim the key on press and swallow the matching release.

---

## 11. Android → Linux mapping (every API touched in scope)

| Android / Kotlin in scope | Linux / Rust replacement | Notes |
|---|---|---|
| `ViewPager` / `DirectionalViewPager` (fork `com.github.tachiyomiorg:DirectionalViewPager:1.0.0`) | `widgets::PagerView` (custom `gtk::Widget`) | Drop the library (T-44a-072). |
| `RecyclerView` + `LinearLayoutManager` + `DiffUtil` + `ViewHolder` | `widgets::WebtoonView` (`gtk::Scrollable`) + `ItemKey` diff | Extra layout space as a realised range (§7.1). |
| `WebtoonLayoutManager` (`scrollToPositionWithOffset`, `findLastEndVisibleItemPosition`) | `webtoon::scroll::last_end_visible`, `WebtoonView::scroll_to(index)` | |
| `SubsamplingScaleImageView` 66e0db195d (tiles, SCALE_TYPE_*, PAN_LIMIT_INSIDE, ZOOM_FOCUS_CENTER, EASE_*) | `zoom::*` pure math + `widgets::PageImage` | Math re-derived from source (p1-topic-zoom-and-image.md). |
| `PhotoView` 2.3.0 (animated pager) | `widgets::PageImage` animated path with `zoom::ZoomState` (levels 1, 2, 5) | |
| `AppCompatImageView` (webtoon animated) | `widgets::PageImage` with zoom disabled | |
| Coil `ImageRequest`, `ViewSizeResolver`, `Precision.INEXACT`, `customDecoder`, `cropBorders` extra | `decode::DecodeRequest { target_width, crop_borders }` | No Coil; no memory/disk cache (B§8.4). |
| `tachiyomi.decoder.ImageDecoder` (native, borders) | `decode::borders::find_borders` (exact port) | Native code not linked. |
| `BitmapFactory`, `Bitmap.compress(JPEG,100)`, `Canvas.drawBitmap` | `image` crate crop, copy and rotate | Lossless (§5.3). |
| `ImageUtil.chooseBackground` (`ImageDecoder`, `Canvas`, `Configuration`) | `decode::background::choose_background` | Landscape check uses the widget size (§5.5). |
| `Handler.postDelayed` / `removeCallbacks` (long tap) | `glib::timeout_add_local_once` with `SourceId` cancel | `gtk-long-press-time` (GTK default 500 ms). |
| `GestureDetector` / `GestureDetectorWithLongTap` | `GestureClick`, `GestureLongPress`, `GestureDrag`, `GestureSwipe` | §10. |
| `ScaleGestureDetector` | `GestureZoom` | Quick scale is built from `GestureClick` + `GestureDrag` (§8.3). |
| `ViewConfiguration.scaledTouchSlop` | `gtk-dnd-drag-threshold` (GtkSettings, default 8) | |
| `ViewConfiguration.getLongPressTimeout()` | `gtk-long-press-time` (GtkSettings) | |
| `ViewConfiguration.getDoubleTapTimeout()` | `gtk-double-click-time` (GtkSettings) | |
| `HapticFeedbackConstants.LONG_PRESS` | none (drop, T-44a-017) | GTK has no haptic API. |
| `ValueAnimator`, `AnimatorSet`, `DecelerateInterpolator` | `zoom::tween` + `add_tick_callback` | §8.5. |
| `Settings.Global.ANIMATOR_DURATION_SCALE` (`animatorDurationScale`) | `gtk-enable-animations` (GtkSettings bool) → scale 0 when false, else 1 | T-44a-074. |
| `Context.resources.displayMetrics.heightPixels` / `widthPixels` | `widget.height()` / `width()` with `scale_factor()` | Recomputed on resize (T-44a-076) instead of cached once (observed quirk, B§7.1). |
| `Resources.getSystem().displayMetrics` (side padding) | widget width | Observed: screen width. Proposal: widget width (small visual change; open question). |
| `ReaderProgressIndicator` (`AbstractComposeView` + Material3) | `gtk::Spinner` (indeterminate) or `gtk::DrawingArea` arc (determinate) | |
| `ReaderTransitionView` (`AbstractComposeView`) + `ChapterTransition` composable | `widgets::TransitionCard` (GTK boxes and labels) | Text rules shared with area:43 (B§11.1). |
| `MaterialButton` / `AppCompatButton` / `ReaderButton` | `gtk::Button` | Gesture gate dropped (§10). |
| `WebViewActivity.newIntent` | `gio::AppInfo::launch_default_for_uri` / `gtk::UriLauncher` | http(s) only (p1-data-and-config.md §5). |
| `ActivityCompat.recreate(activity)` (webtoon theme change) | in-place CSS provider swap (`gtk::CssProvider`) | T-44a-043. |
| `isNightMode()` / `ThemeMode` / `UI_MODE_NIGHT_*` (`createReaderThemeContext`) | `gtk::Settings::gtk_application_prefer_dark_theme` + `org.freedesktop.appearance color-scheme` (portal) | Theme 3 "Automatic" uses the desktop colour scheme (T-44a-077). |
| `Dispatchers.Main` (`MainScope`) | `glib::MainContext` (`spawn_future_local`) | §4. |
| `Dispatchers.IO` / `launchIO` / `withIOContext` | `tokio::task::spawn_blocking` / `tokio::spawn` | §4. |
| `StateFlow` / `MutableStateFlow` (`Page.statusFlow`, `progressFlow`, `ReaderChapter.stateFlow`) | `tokio::sync::watch` | §3. |
| `collectLatest` | `watch::changed()` loop with `select!` on cancel | Cancels the previous branch (B§12.3). |
| `logcat { }` | `tracing::debug!` (T-44a-083) | |
| `activity.runOnUiThread` | `glib::idle_add_local_once` or direct call on main thread | §4. |
| `Injekt.get<ReaderPreferences>()` | `ViewerSettings` passed by constructor | No global DI. |

---

## 12. Integration with the existing mockup

Current state (read in full): `src/player.rs` lines 49-147 `pub fn reader(title, chapter) -> GBox` builds a `gtk::Overlay` with a placeholder cover canvas (`cover(420, 600)`), top OSD bar with back button, bookmark toggle and menu button, bottom chapter navigator (`gtk::Scale` 1..20, placeholder), bottom bar (reading mode, rotation, crop toggle, settings), an indicator label, and a `gtk::GestureClick` on the canvas where x < 1/3 w → previous page, x > 2/3 w → next page, middle → toggle menus. A long press opens `page_actions` dialog. Mock sheets come from `spec/reader_player.json` (reader tab "Reading mode" with chips, "Paged" and "Long strip" groups, "General", "Custom filter").

Replacement plan (proposal):
1. `src/player.rs::reader()` becomes a thin shell: keep the overlay, top bar, bottom bar and page indicator (area:43 chrome). Replace the canvas and its `GestureClick` with `reader::ReaderView`, which owns the viewer widget and the input wiring of §10.
2. `reader::ReaderView::new(kind, chapters_rx, events_tx)` returns a `gtk::Widget` added as the overlay child.
3. The mock's fixed 1/3-zone tap handler is removed. Tap zones come from `nav::layouts` per the active navigation mode (default LNavigation/RightAndLeft, §2.3 of p1-behavior.md). The mock's zones (x < 1/3 prev, x > 2/3 next) are the RightAndLeft layout only for L2R.
4. `main.rs:54-58` (`immersive` check by widget name `screen:reader`) stays unchanged; `ReaderView` is placed inside the `screen:reader` tagged box.
5. Settings chips in `spec/reader_player.json` must use the *stored values* from §1.3 of p1-behavior.md. The mock's "Background color" chip order (Black, Gray, White, Auto) matches the Kotlin menu order and must not be reordered (verified against `SettingsReaderScreen.kt:92-97`).
6. The mock's `scale` slider (1..20) becomes a real page slider bound to `ViewerEvent::PageSelected` and to `move_to_page` (B§6.5 slider semantics: sets `isScrollingThroughPages`, see open question 3).

---

## 13. Error handling

- Decode errors: `DecodeError` → `PageStatus::Error` on the page watch (B§8.2 catch-all → setError). The error card shows "The image couldn't be loaded" (`decode_image_error`).
- Loader errors: area:43 sets `PageStatus::Error`; the card is identical.
- Transition load errors: `ChapterLoadState::Error(msg)` → transition card shows "Failed to load pages: {msg}" (B§11.3).
- Missing stream (`PageRef.stream == None`): no state change, page stays blank (B§8.2 step 2). Logged at `warn`.
- Panics: none in the viewer code. `unwrap`/`expect` only in tests. All index arithmetic is saturating or checked (B§12.4 hazards).
- Preference parse errors: unknown integer → default (B§1.3).

---

## 14. Pitfalls (must be handled by the implementer)

1. **Texture creation off the main thread** panics or silently fails in gtk4-rs. Only `RgbaBuffer` crosses threads.
2. **`gdk::MemoryTexture` stride** must be `width * 4` for RGBA8 when the buffer is tightly packed. Pass the stride explicitly from `RgbaBuffer`.
3. **GL texture size**: keep tiles at or below 2048 px (B§10.1) regardless of the device limit. A larger device limit is not needed to match the observed behaviour.
4. **Scale factor**: all pixel metrics (20 dp threshold, 15 dp gap, 64/128/32 dp paddings, 16 dp margins) are multiplied by `widget.scale_factor()`.
5. **Integer division**: `scroll_distance`, `half_w`, `half_h`, `split width` use integer division in the source. Use integer division where the source does, floats elsewhere.
6. **Equality of transitions** (§3.2): `Prev(A,B) == Next(B,A)`. Test it (E-03).
7. **Key claim/release**: claim on press, swallow matching release (B§4.2 returns true on both edges).
8. **Scroll wheel sign**: GTK `dy > 0` is down. Map to next page. `dy == 0` maps to previous (observed behaviour, B§4.3).
9. **Frame clock**: `add_tick_callback` returns `TickCallbackId`; remove the callback on completion (`ControlFlow::Break`), or it leaks per animation.
10. **Widget unrealise** during a decode: check the generation counter before applying the texture (§4).
11. **`glib::spawn_future_local`** panics when called off the main thread. Dropping a `glib::JoinHandle` does not stop the task. Stop status and progress loops with a `CancellationToken` checked in the loop.
12. **Double-click window** for webtoon single tap: GTK delivers `n_press` per click; the single tap is emitted after `gtk-double-click-time` only if no second press arrives. Use `glib::timeout_add_local_once` and cancel it on the second press.
13. **Pan with `PAN_LIMIT_INSIDE` on small images**: when the scaled image is smaller than the view, centre it; the Kotlin `getPanRemaining` then gives 0 and `canPan` is false (B§10.4).
14. **Animated frame delays** of 0 or below 10 ms: browsers clamp; the Kotlin/Coil path uses the decoder's delay. Clamp to 10 ms minimum (documented proposal; prevents busy loops).

---

## 15. Crates (decision list)

| Crate | Use | Why |
|---|---|---|
| `gtk4` 0.11 (already) | widgets, gestures, snapshot, tick callbacks | Given. |
| `glib` / `gdk` / `gsk` / `graphene` (re-exported by gtk4) | textures, scaling filter, geometry | Re-exported; no separate version pin. |
| `tokio` (already in plan) | blocking pool, watch channels, cancellation | Decode and loads. |
| `tokio-util` | `CancellationToken` | Cancel on unbind. |
| `async-channel` | decode results to GTK thread | Works with `glib::spawn_future_local`. |
| `image` 0.25 (features: `jpeg`, `png`, `webp`, `gif`, `avif`) | decode, transform, GIF/WebP frames | The baseline decoder. |
| `zune-jpeg` (optional) | faster JPEG | Only if profiling needs it (T-44a-050). |
| `jxl-oxide` | JXL decode | Pure Rust, matches the observed JXL support. |
| `libheif-rs` (optional, feature `heif`) | HEIF | Needs system libheif; off by default. |
| `fast_image_resize` (optional) | webtoon target-width downscale | Only if `image` resize is too slow (T-44a-050). |
| `lru` | decoded page cache keys | Byte budget implemented on top (`lru.rs`). |
| `serde` / `serde_json` (already) | settings and fixtures | Given. |
| `tracing` | logs (replaces logcat) | T-44a-083. |
| `proptest` (dev) | zoom and geometry invariants | Optional; unit tests are enough for P0. |
| `insta` (dev, optional) | snapshot tests for item lists | Optional; plain `assert_eq!` is the default. |

Avoid: a GTK paging library (none is mature), `adw` (libadwaita is excluded by the brief), any video or player crate here (not in this area).

---

## 16. Build and test notes

- The pure modules compile without GTK. Keep them free of `gtk::` imports so `cargo test --lib reader::pager::items` runs headless.
- Use `cargo test` with `GDK_BACKEND=broadway` or none; widget tests (if any) need a display. Keep them to the golden layout (T-44a-080 uses pure math only).
- Fixtures: small PNG/JPEG/GIF/WebP files under `tests/fixtures/reader/` (p1-edge-cases-and-tests.md §4).
