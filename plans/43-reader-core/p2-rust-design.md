# 43-reader-core (part 2 of 2) — Rust design

Target: Kurobayomi (`/home/blackbox/kurobayomi`), Rust 2024 edition, gtk4-rs 0.11 (`gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, present in `Cargo.toml`), no libadwaita, vanilla GTK 4.16+ theming. Behavior source of truth: `p2-behavior.md`. Data: `p2-data-and-config.md`.

Contents
1. Current mockup and what it means for this area
2. Module layout
3. Core types and trait signatures
4. Threading model (GTK main loop vs tokio)
5. Settings store and change bus
6. Page view: widget, zoom model, gestures
7. Decoding and textures
8. Navigation, gestures and input
9. Overlays: progress, transition, flash, buttons
10. Crates and why
11. Error handling
12. Android API to Linux mapping
13. Pitfalls
14. Integration points in the mockup (src/player.rs, src/ui.rs, src/spec.rs, src/main.rs, spec/reader_player.json)

---

## 1. Current mockup

- `src/player.rs:49-149` `pub fn reader(title: &str, chapter: &str) -> GBox` builds a static page: a `gtk::Overlay` with a centered `cover(420, 600)` placeholder (`src/ui.rs:177`), an OSD top bar (back button calls `pop()`, title via `two_line`, bookmark `ToggleButton`, `menu_button` with `screen("reader").menu`), and a comment at line 122 `// center tap toggles menus, side taps turn pages`. There is no page decoding and no zoom.
- `src/player.rs:150-153` `fmt_time` and `src/player.rs:154+` `pub fn player(...)` belong to the anime player (other area). Do not touch.
- `src/main.rs:54-58` hides the sidebar and tab bar for `screen:reader` and `screen:player` (immersive). Keep.
- `src/screens.rs:37` routes `"reader"` to `crate::player::reader("Title 1", "Chapter 1")` and `src/screens.rs:67` picks reader vs player by kind.
- `src/spec.rs:126` includes `spec/reader_player.json` into the spec loader; the reader screen's settings sheet (labels "Reading mode", "Rotation", "Tap zones", "Invert tap zones", "Scale type", "Zoom start position", "Crop borders", "Automatically zoom into wide images", "Pan wide images", "Split wide pages", "Invert split page placement", "Rotate wide pages to fit", "Flip orientation of rotated wide pages", "Side padding", "Double tap to zoom", "Disable zoom out", "Show page number", "Fullscreen", "Show content in cutout area", "Keep screen on", "Show actions on long tap", "Always show chapter transition", "Animate page transitions", "Flash on page change", "Flash duration", "Flash every", "Flash with", "Custom brightness", "Custom color filter", "Red", "Green"...) is rendered from JSON by the generic spec renderer (`src/ui.rs:221 item_row`, `src/settings.rs`).
- `src/ui.rs:12 with_spec`, `src/ui.rs:88 open`, `src/ui.rs:391 show_sheet`, `src/ui.rs:406 show_dialog` drive these sheets from the spec. Values in the mockup are not stored anywhere; slider and switch rows are not wired to a store.

Implication: this area replaces the static `reader()` body with a real reader (the `ReaderView` widget described below), backed by a typed prefs store. The spec-rendered settings sheet stays as the UI shell, but its items must read/write `ReaderPrefs` (T-43b-031). `spec/reader_player.json` keeps the labels; Aniyomi's exact labels are the source for these (see `p2-behavior.md` §15).

## 2. Module layout (new files)

```
src/reader/mod.rs            pub mod re-exports; ReaderView factory; ReaderSession (window-level state)
src/reader/prefs.rs          ReaderPrefs (typed getters/setters, keys, defaults), Pref<T> handle
src/reader/enums.rs          FlashColor, TappingInvertMode, ReaderHideThreshold, ZoomStart,
                             ImageScaleType, ColorFilterMode, TapZoneLayout (index list), labels
src/reader/mode.rs           ReadingMode (flag values, MASK, direction, viewer kind), resolve + factory
src/reader/nav.rs            NavRegion (MENU, PREV, NEXT, LEFT, RIGHT), Region, invert(), action_at()
src/reader/gesture.rs        LongPressTracker (pure state machine, no GTK types), tests
src/reader/chapter_gap.rs    calculate_chapter_gap(higher: Option<&ChapterRef>, lower: Option<&ChapterRef>) -> i32
src/reader/viewer.rs         trait ReaderViewer (the Viewer contract)
src/reader/viewer_config.rs  ViewerConfig: field set + subscriptions (pref -> field, then onChanged)
src/reader/page_zoom.rs      ZoomModel: fit scale per ImageScaleType, min/max, zoom start anchors,
                             landscape auto-zoom decision, pan step math (pure, unit tested)
src/reader/page_decode.rs    decode pipeline: bytes -> RGBA8 (image crate), hardware threshold / tiling
                             decision, crop borders, dual-page split math
src/reader/page_view.rs     PageView (glib subclass of gtk::Widget): snapshot(), gestures, load state
src/reader/progress.rs      LoadingIndicator widget (spinner + optional percent)
src/reader/transition.rs    ChapterTransitionView widget (binds two booleans + transition model)
src/reader/overlay_button.rs  ReaderButton equivalent (gtk::Button with no pager gesture coupling)
src/reader/flash.rs         FlashHost: counter, interval rule, color steps (widget drawing via snapshot)
src/reader/settings_model.rs ReaderSettingsModel: current viewer kind + manga flags + callbacks (glib signals)
```

Rule: pure logic lives in modules that do not import `gtk` (`prefs`, `enums`, `mode`, `nav`, `gesture`, `chapter_gap`, `page_zoom`, the decision parts of `page_decode`). They get `cargo test` coverage (see p2-edge-cases-and-tests.md). GTK code is in `page_view`, `progress`, `transition`, `overlay_button`, `flash` drawing, `settings_model` signal glue.

## 3. Core types and trait signatures

### 3.1 Preferences

```rust
// src/reader/prefs.rs
use crate::reader::enums::*;
use crate::reader::mode::ReadingMode;

/// Typed handle to one preference row. Reads are cheap (in-memory cache).
pub struct Pref<T: PrefValue> {
    key: &'static str,
    default: T,
    store: std::rc::Rc<PrefStore>,
}

pub trait PrefValue: Clone + PartialEq + 'static {
    fn encode(&self) -> String;
    fn decode(raw: &str) -> Option<Self>;
}

impl<T: PrefValue> Pref<T> {
    pub fn get(&self) -> T;                       // default on missing or invalid (and deletes invalid row)
    pub fn set(&self, value: T);                  // writes cache, persists async, notifies subscribers
    pub fn delete(&self);
    pub fn is_set(&self) -> bool;
    pub fn default_value(&self) -> &T;
    /// Mirrors Preference.changes(): calls `f` now with the current value, then on each change
    /// where the value differs from the last delivered value (distinctUntilChanged).
    pub fn subscribe(&self, f: impl Fn(&T) + 'static) -> Subscription;
}

pub struct ReaderPrefs { /* one Pref<_> field per key in p2-data-and-config.md §2 */ }

impl ReaderPrefs {
    pub const WEBTOON_PADDING_MIN: i32 = 0;
    pub const WEBTOON_PADDING_MAX: i32 = 25;
    pub const MILLI_CONVERSION: i32 = 100;
    pub fn page_transitions(&self) -> Pref<bool>;          // key "pref_enable_transitions_key", default true
    pub fn flash_on_page_change(&self) -> Pref<bool>;      // "pref_reader_flash", false
    pub fn flash_duration_millis(&self) -> Pref<i32>;      // "pref_reader_flash_duration", 100
    pub fn flash_page_interval(&self) -> Pref<i32>;        // "pref_reader_flash_interval", 1
    pub fn flash_color(&self) -> Pref<FlashColor>;         // "pref_reader_flash_mode", BLACK
    pub fn double_tap_anim_speed(&self) -> Pref<i32>;      // "pref_double_tap_anim_speed", 500
    // ... one accessor per key; names mirror the Kotlin getters, snake_cased
}
```

`PrefStore` = the SQLite `kv` table with an in-memory `HashMap<String, String>` cache loaded at startup (see §5).

Enum encoding: `FlashColor`, `TappingInvertMode`, `ReaderHideThreshold` encode as the Kotlin enum names (`"BLACK"`, `"NONE"`, `"LOW"`, …) so values match Aniyomi backups.
Int encoding: decimal. Bool encoding: `"true"` / `"false"`.

### 3.2 Enums

```rust
// src/reader/enums.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashColor { Black, White, WhiteBlack }          // names BLACK, WHITE, WHITE_BLACK
impl FlashColor { pub const DEFAULT: Self = Self::Black; pub fn name(self) -> &'static str; pub fn parse(s: &str) -> Option<Self>; }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TappingInvertMode { None, Horizontal, Vertical, Both }
impl TappingInvertMode {
    pub fn invert_horizontal(self) -> bool;   // Horizontal | Both
    pub fn invert_vertical(self) -> bool;     // Vertical | Both
    pub fn label_key(self) -> &'static str;   // "tapping_inverted_none" ...
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderHideThreshold { Highest, High, Low, Lowest }   // 5, 13, 31, 47 px
impl ReaderHideThreshold { pub fn px(self) -> i32; pub const DEFAULT: Self = Self::Low; }

/// Index list. Stored value = index + 1 (1-based) for scale type and zoom start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageScaleType { FitScreen = 1, Stretch = 2, FitWidth = 3, FitHeight = 4, OriginalSize = 5, SmartFit = 6 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomStart { Automatic = 1, Left = 2, Right = 3, Center = 4 }
/// Tap zone layouts: index 0..=5 stored in reader_navigation_mode_*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapZoneLayout { Default = 0, LShaped = 1, KindleIsh = 2, Edge = 3, RightAndLeft = 4, Disabled = 5 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorFilterBlend { SrcOver = 0, Multiply = 1, Screen = 2, Overlay = 3, Lighten = 4, Darken = 5 }
```

`ImageScaleType::from_stored(i32) -> ImageScaleType` returns FitScreen for values outside 1..=6 **only as a fallback in the Rust layer**; the Kotlin layer would throw (behavior §3.5). Decision recorded in T-43b-015.

### 3.3 Reading mode

```rust
// src/reader/mode.rs
pub const READING_MODE_MASK: i64 = 0x7;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingMode { Default = 0, LeftToRight = 1, RightToLeft = 2, Vertical = 3, Webtoon = 4, ContinuousVertical = 5 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction { Horizontal, Vertical }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerKind { Pager, Webtoon }

impl ReadingMode {
    pub fn from_stored(v: Option<i64>) -> ReadingMode;   // unknown / None -> Default
    pub fn direction(self) -> Option<Direction>;         // None for Default
    pub fn viewer_kind(self) -> Option<ViewerKind>;      // None for Default
    pub fn is_pager(stored: i64) -> bool;                // Kotlin ReadingMode.isPagerType
    /// Resolve Default to the global default before viewer creation (behavior §4.3).
    pub fn resolve(self, global_default: ReadingMode) -> ReadingMode;
}

/// Manga-level helpers on the flag word (bits 0..2).
pub fn manga_reading_mode(viewer_flags: i64) -> i64 { viewer_flags & READING_MODE_MASK }
pub fn with_reading_mode(viewer_flags: i64, mode: ReadingMode) -> i64 {
    (viewer_flags & !READING_MODE_MASK) | (mode as i64 & READING_MODE_MASK)
}
```

Note: `resolve` must never return `Default`. Viewer factory (below) returns `Err` for `Default` in Rust (the Kotlin code throws `IllegalStateException`).

### 3.4 Navigation

```rust
// src/reader/nav.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavRegion { Menu, Prev, Next, Left, Right }
impl NavRegion {
    pub fn label_key(self) -> &'static str;     // "action_menu", "nav_zone_prev", "nav_zone_next", "nav_zone_left", "nav_zone_right"
    pub fn argb(self) -> u32;                   // 0xCC_95818D (Menu), 0xCC_FF7733 (Prev), 0xCC_84E296 (Next), 0xCC_7D1128 (Left), 0xCC_A6CFD5 (Right)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectN { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 } // normalized [0,1]
impl RectN {
    /// Android RectF.contains semantics: half-open, empty rect contains nothing.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.left < self.right && self.top < self.bottom
            && x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
    pub fn invert(self, h: bool, v: bool) -> RectN;   // rules in p2-behavior.md §7.3
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region { pub rect: RectN, pub kind: NavRegion }

pub trait TapLayout {
    fn regions(&self) -> &[Region];   // implemented in area 44 (L shaped, Kindle-ish, Edge, Right and Left, Disabled, Default)
}

/// Mirrors ViewerNavigation.getAction. Inversion applied per call (cheap).
pub fn action_at(regions: &[Region], invert_h: bool, invert_v: bool, x: f32, y: f32) -> NavRegion {
    regions.iter()
        .map(|r| Region { rect: r.rect.invert(invert_h, invert_v), kind: r.kind })
        .find(|r| r.rect.contains(x, y))
        .map(|r| r.kind)
        .unwrap_or(NavRegion::Menu)   // both Kotlin branches return MENU; constantMenuRegion is dead
}
```

Rust decision: keep the outer `Menu` fallback; drop `constantMenuRegion` (behavior quirk 2).

### 3.5 Long-press tracker (pure)

```rust
// src/reader/gesture.rs
pub struct LongPressTracker {
    pub long_press_ms: u32,          // from gtk-long-press-time (default 500)
    pub double_tap_ms: u32,          // from gtk-double-click-time (default 400)? see p2-behavior §8 proposal
    pub slop_px: f64,                // 8.0 default
    last_up_ms: Option<u32>,
    down_at: Option<(f64, f64)>,     // position of the armed press
    armed: bool,
}

pub enum LongPressAction { Arm { after_ms: u32 }, Disarm, None }

impl LongPressTracker {
    pub fn on_press(&mut self, t_ms: u32, x: f64, y: f64) -> LongPressAction;
    pub fn on_motion(&mut self, x: f64, y: f64) -> LongPressAction;  // Disarm when |dx|>slop or |dy|>slop
    pub fn on_release(&mut self, t_ms: u32) -> LongPressAction;      // records last_up_ms, Disarm
    pub fn on_cancel(&mut self) -> LongPressAction;                  // Disarm (also for second pointer)
    pub fn fire(&mut self) -> bool;                                   // called by the timer; true => long press
}
```

`on_press` arms only when `t_ms - last_up_ms.unwrap_or(0) > double_tap_ms` (behavior §8.2). The timer is a `glib::timeout_add_local_once(Duration::from_millis(after_ms))` created by the widget; `fire` checks `armed` to ignore stale timers.

### 3.6 Viewer contract

```rust
// src/reader/viewer.rs
pub trait ReaderViewer {
    fn widget(&self) -> &gtk::Widget;
    fn destroy(&mut self) {}                             // default no-op (Kotlin default)
    fn set_chapters(&mut self, chapters: &ViewerChapters);   // model from part 1
    fn move_to_page(&mut self, page: PageRef);           // PageRef from part 1 model
    /// Returns true if handled (Kotlin Viewer.handleKeyEvent)
    fn handle_key(&mut self, key: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool;
    /// Mapping of Android generic-motion (scroll/wheel) events
    fn handle_scroll(&mut self, dx: f64, dy: f64, state: gtk::gdk::ModifierType) -> bool;
}
```

`ViewerChapters` and `PageRef` are defined in part 1 (`src/reader/model.rs`, owned by the other author of area 43). This file references them by name only.

### 3.7 ViewerConfig

```rust
// src/reader/viewer_config.rs
pub struct ViewerConfig {
    pub tapping_inverted: TappingInvertMode,      // default None
    pub long_tap_enabled: bool,                   // default true
    pub use_page_transitions: bool,               // default FALSE until first emission (behavior §6.1)
    pub double_tap_anim_duration: i32,            // default 500
    pub volume_keys_enabled: bool,                // default false
    pub volume_keys_inverted: bool,               // default false
    pub always_show_chapter_transition: bool,     // default true
    pub navigation_mode: TapZoneLayout,           // default Default (0)
    pub force_navigation_overlay: bool,           // from one-shot pref
    pub navigation_overlay_on_start: bool,        // default false
    pub dual_page_split: bool, pub dual_page_invert: bool,
    pub dual_page_rotate_to_fit: bool, pub dual_page_rotate_to_fit_invert: bool,
}

impl ViewerConfig {
    /// Builds the struct and wires every subscription. The one-shot overlay pref is cleared here.
    pub fn new(prefs: &ReaderPrefs) -> ViewerConfig;   // subscriptions via Rc<RefCell<ViewerConfig>>
}
```

Subscriptions: each `Pref::subscribe` runs immediately (ignition) and on changes. Callbacks write the field and then call the viewer's `on_changed` closure (equivalent to `register(valueAssignment, onChanged)`). Store `Subscription` handles in the viewer so they drop with it (replaces `launchIn(scope)`; cancelling the scope = dropping the subscription).

### 3.8 Page view

```rust
// src/reader/page_view.rs (glib subclass)
glib::wrapper! {
    pub struct PageView(ObjectSubclass<imp::PageView>) @extends gtk::Widget;
}

pub struct PageConfig {               // == ReaderPageImageView.Config
    pub zoom_duration_ms: i32,        // pref value; scaled by system animation factor (see §12)
    pub min_scale: ImageScaleType,    // minimumScaleType (default FitScreen)
    pub crop_borders: bool,
    pub zoom_start: ZoomStartPos,     // Left | Center | Right
    pub landscape_zoom: bool,
    pub is_webtoon: bool,
}
pub enum ZoomStartPos { Left, Center, Right }

impl PageView {
    pub fn new(is_webtoon: bool) -> PageView;
    pub fn set_image(&self, image: DecodedPage, config: PageConfig);   // DecodedPage: RGBA + kind (still | animated)
    pub fn recycle(&self);                                              // drop texture, hide (Kotlin recycle())
    pub fn on_page_selected(&self, forward: bool);                     // behavior §9.7
    pub fn can_pan_left(&self) -> bool;  pub fn can_pan_right(&self) -> bool;   // >1 px remaining
    pub fn pan_left(&self);  pub fn pan_right(&self);                  // one view width, 250 ms ease-out
    // callbacks (Kotlin onImageLoaded / onImageLoadError / onScaleChanged / onViewClicked)
    pub fn connect_image_loaded<F: Fn() + 'static>(&self, f: F) -> glib::SignalHandlerId;
    pub fn connect_image_load_error<F: Fn() + 'static>(&self, f: F) -> glib::SignalHandlerId;
    pub fn connect_scale_changed<F: Fn(f64) + 'static>(&self, f: F) -> glib::SignalHandlerId;
    pub fn connect_view_clicked<F: Fn() + 'static>(&self, f: F) -> glib::SignalHandlerId;
    pub fn set_page_background(&self, rgba: Option<gtk::gdk::RGBA>);   // pageBackground
}
```

Signals are GLib signals (`image-loaded`, `image-load-error`, `scale-changed`, `view-clicked`) declared in `imp` with `glib::subclass::Signal`. Using signals avoids the Kotlin pattern of nullable lambdas that can be overwritten.

### 3.9 Zoom model (pure)

```rust
// src/reader/page_zoom.rs
#[derive(Clone, Copy, Debug)]
pub struct Size { pub w: f64, pub h: f64 }

/// Minimum scale per SSIV.minScale() (fork SubsamplingScaleImageView.java:1904-1931).
pub fn min_scale(kind: ImageScaleType, view: Size, src: Size) -> f64 {
    let fit_w = view.w / src.w;
    let fit_h = view.h / src.h;
    match kind {
        ImageScaleType::FitScreen => fit_w.min(fit_h),          // SCALE_TYPE_CENTER_INSIDE (also the default branch)
        ImageScaleType::Stretch => fit_w.max(fit_h),            // SCALE_TYPE_CENTER_CROP (= crop-to-fill)
        ImageScaleType::FitWidth => fit_w,
        ImageScaleType::FitHeight => fit_h,
        ImageScaleType::OriginalSize => 1.0,
        ImageScaleType::SmartFit => if src.h > src.w { fit_w } else { fit_h },
    }
}

pub const MAX_ZOOM_FACTOR: f64 = 5.0;        // maxScale = minScale * 5 (behavior §9.5)
pub const DOUBLE_TAP_ZOOM_FACTOR: f64 = 2.0; // setDoubleTapZoomScale(scale * 2)
pub fn clamp_scale(s: f64, min: f64) -> f64 { s.clamp(min, min * MAX_ZOOM_FACTOR) } // SSIV limitedScale: max(minScale, target); max bound from setupZoom

/// Landscape auto-zoom precondition (behavior §9.6, all five terms).
pub fn landscape_zoom_applies(cfg_landscape_zoom: bool, kind: ImageScaleType, src: Size, current: f64, min: f64) -> bool {
    cfg_landscape_zoom && kind == ImageScaleType::FitScreen && src.w > src.h && (current - min).abs() < f64::EPSILON
}
pub fn landscape_target_scale(view_h: f64, src_h: f64) -> f64 { view_h / src_h }   // fit image height
pub const LANDSCAPE_ZOOM_DELAY_MS: u32 = 500;
pub const LANDSCAPE_ZOOM_DURATION_MS: u32 = 500;
pub const PAN_DURATION_MS: u32 = 250;
```

Center anchors (behavior §9.5/§9.6): `Left` → x=0, `Right` → x=src_w, `Center` → current center; y is 0 for Left/Right.

### 3.10 Decode pipeline

```rust
// src/reader/page_decode.rs
pub struct DecodedPage {
    pub width: u32, pub height: u32,
    pub rgba: glib::Bytes,            // premultiplied? no: straight RGBA8, row stride = width * 4
    pub animated: Option<AnimatedFrames>,   // Some for GIF/APNG/animated WebP
}

pub struct AnimatedFrames { pub frames: Vec<(gtk::gdk::MemoryTexture, u32 /*delay ms*/)> }

/// Decision: whether to keep the full bitmap or tile (behavior §9.3 threshold rule).
pub fn needs_tiling(width: u32, height: u32, threshold: u32) -> bool {  // max(w,h) > threshold
    width.max(height) > threshold
}
pub const DEFAULT_HW_THRESHOLD: u32 = 2048;   // GLUtil.SAFE_TEXTURE_LIMIT

pub fn decode_page(bytes: &[u8], opts: DecodeOptions) -> Result<DecodedPage, DecodeError>;
pub struct DecodeOptions { pub max_dimension: u32, pub crop_borders: bool, pub animated_ok: bool }
pub fn split_wide(page: &DecodedPage, invert: bool) -> (DecodedPage, DecodedPage);  // dual page (P1)
pub fn rotate_to_fit(page: DecodedPage, clockwise: bool) -> DecodedPage;            // rotate wide page (P1)
```

`decode_page` runs on the blocking pool only (§4).

### 3.11 Progress, transition, flash, overlay button

```rust
// src/reader/progress.rs
pub struct LoadingIndicator(/* gtk::Box with gtk::Spinner + optional gtk::Label percent */);
impl LoadingIndicator {
    pub fn show(&self);                 // set_visible(true)
    pub fn hide(&self);                 // set_visible(false)
    pub fn set_progress(&self, percent: u8);   // clamp 0..=100 in Rust (Kotlin: no clamp, annotation only)
}

// src/reader/transition.rs
pub struct ChapterTransitionView(/* gtk::Box */);
impl ChapterTransitionView {
    /// Equivalent of ReaderTransitionView.bind(transition, downloadManager, manga):
    /// None => clear; Some(manga) => compute the two downloaded flags.
    pub fn bind(&self, transition: &ChapterTransition, manga: Option<&MangaRef>, dl: &DownloadIndex);
}

// src/reader/flash.rs
pub struct FlashHost { /* counter, interval, color, duration, on_draw: gtk::DrawingArea */ }
impl FlashHost {
    pub fn flash(&mut self);                     // page changed: counter rule (behavior §14)
    pub fn set_interval(&mut self, interval: u32); // resets counter to 0
    pub fn is_active(&self) -> bool;
}

// src/reader/overlay_button.rs
pub fn overlay_button(label: &str, icon: Option<&str>) -> gtk::Button;   // plain gtk::Button; no pager coupling
```

## 4. Threading model

- GTK main thread (GLib default main context): all widgets, `gdk::Texture` creation (`gdk::MemoryTexture::new` is fine on main thread), signal emission, prefs cache mutation, viewer state.
- tokio runtime (multi-thread, started once in `main.rs` or a `OnceLock`): network and DB work owned by other areas. Reader calls into the loader through async APIs from part 1.
- Image decode: `tokio::task::spawn_blocking(move || decode_page(bytes, opts))` (CPU-bound). Results return to the GTK thread via `glib::spawn_future_local(async move { let page = join.await?; page_view.set_image(page, cfg); })`. Never block the main thread on decode.
- Preloading: the viewer requests the next N pages (N = 2 default, decided in part 1 / area 44) by spawning decode jobs keyed by page index; results are cached in a small LRU keyed by `(chapter_id, page_index, opts_hash)`. Cancel jobs whose page is no longer in the window (drop the `JoinHandle` results; `spawn_blocking` cannot be cancelled, so the result is discarded on arrival).
- Timers: `glib::timeout_add_local_once` for the long-press timer, landscape-zoom delay (500 ms), flash half-steps (`duration/2`). No tokio timers on the GTK thread.
- Animations: `gtk::Adjustment` / `glib::timeout` driven easing for zoom/pan (EASE_IN_OUT_QUAD, EASE_OUT_QUAD). Honor `gtk-enable-animations` (see §12).
- Prefs persistence: `Pref::set` updates the in-memory cache synchronously, then sends `(key, value)` over a `std::sync::mpsc` channel to a tokio task that does the SQLite write. Reads never touch SQLite.

## 5. Settings store and change bus

- Table: `kv(key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL) STRICT` (see p2-data-and-config.md §7).
- On startup: `SELECT key, value FROM kv` loads every row into the cache (the table holds one row per changed preference, so it stays small).
- Subscription = `Pref::subscribe` (behavior §6.2 semantics). Implementation: `Rc<RefCell<Vec<Subscriber>>>` per key on the GTK thread; `set` iterates subscribers and skips those whose last delivered value equals the new value (distinctUntilChanged).
- Cross-thread: none needed for reader prefs; the settings sheet and reader share one `Rc<ReaderPrefs>` on the GTK thread.
- Backup/restore: area 21/22 read/write the same key strings through `PrefStore` (no reader-specific code here).

## 6. Page view: widget, zoom model, gestures

### 6.1 Widget structure
`PageView` is a `gtk::Widget` subclass (not a `gtk::Picture`), because zoom/pan needs a custom transform and a texture drawn with `gtk::Snapshot`:
- `snapshot()`: `snapshot.translate(origin)`, `snapshot.scale(zoom)`, `snapshot.append_texture(&texture, &rect)` with `texture_rect = (0, 0, src_w, src_h)`. Clip to the widget bounds.
- `measure()`: natural size = view size (the page fills the reader area; `hexpand` and `vexpand` true).
- For animated pages: `set_frame(index)` swaps the texture from `AnimatedFrames` on a `glib::timeout_add_local_once(delay)` chain; `queue_draw()`.
- State fields: `texture: Option<gdk::Texture>`, `src: Size`, `zoom: f64`, `center: (f64, f64)` (source pixels, mirrors SSIV `center`), `min_scale`, `max_scale`, `kind: ImageScaleType`, `config: PageConfig`, `state: LoadState` (Empty | Loading | Ready | Error).

### 6.2 Zoom and pan model
- Units: `zoom` = screen px per source px. `center` in source px.
- Fitted scale at ready: `s = min_scale(kind, view, src)`; `zoom = s`; `min_scale = s`; `max_scale = s * MAX_ZOOM_FACTOR`.
- Zoom start at ready (behavior §9.5): Left → center = (0, 0); Right → center = (src_w, 0); Center → center = (src_w/2, src_h/2). The Kotlin code calls `setScaleAndCenter(scale, PointF(0,0))` for Left, which anchors the top-left corner, not the vertical middle. Keep that.
- `clamp_scale` for user zoom: `[min_scale, min_scale * 5]`.
- Double-tap (non-animated, Kotlin SSIV): zoom to `min_scale * 2` centered on the tap (`ZOOM_FOCUS_CENTER`) with duration `zoom_duration * system factor`; from there, double-tap again returns to min. Replicate: if `zoom > min_scale` → animate to min; else animate to `min*2` focused on the tap point. Note the Kotlin pair is `setDoubleTapZoomScale(scale*2)` plus SSIV's own double-tap toggle; treat as "toggle between min and 2x min".
- Double-tap (animated, Kotlin PhotoView): scale levels 1, 2, 5 (`setScaleLevels(1,2,5)`); double tap: if `scale > 1` → 1 else 2 (absolute, not multiplied by fit scale). Keep: animated pages use absolute levels.
- Pan clamp (PAN_LIMIT_INSIDE): the image cannot be dragged past its edges except the extra space SSIV adds when the image is smaller than the view. Replicate by clamping `center` so the scaled image covers the view in each axis when `scaled_w >= view_w`, else center it.
- `can_pan_left()` = remaining pan toward left in view px > 1 (behavior §9.9).
- `pan_left()`/`pan_right()`: target center `x -= view.w / zoom` (or `+=`), animated over `PAN_DURATION_MS = 250` with ease-out quad, interruptible by user gesture.

### 6.3 Gestures (GTK4 controllers)
| Gesture | Controller | Behavior |
|---|---|---|
| single click | `GestureClick` (`n_press == 1`, delayed by double-click time so a double click can cancel) | emit `view-clicked` (tap zones handled by the viewer, area 44) |
| double click | `GestureClick` (`n_press == 2`) | double-tap zoom (above) |
| drag | `GestureDrag` | pan by delta when zoomed; when not zoomed (zoom == min) pass through (no pan) |
| pinch | `GestureZoom` | `zoom = clamp_scale(start_zoom * scale, min)`; keep focal point fixed |
| ctrl+wheel | `EventControllerScroll` (`VERTICAL`, state CONTROL) | zoom step 1.1x per tick about pointer (mouse equivalent of pinch) |
| plain wheel | `EventControllerScroll` | viewer (page turn / vertical scroll), not this widget |
| long press | `LongPressTracker` (§3.5) fed from `GestureClick` press/release and `GestureDrag` motion | emit viewer long-tap with the press position |

Gesture coexistence: `GestureClick` and `GestureLongPress` are not used together (the tracker covers long press so double-tap guard matches Kotlin). `GestureZoom` and `GestureDrag` run `PropagationPhase::Capture` with `set_state(GestureClaim)` when zooming to prevent the pager from swiping.

### 6.4 Landscape auto-zoom
On `on_page_selected(forward)` or ready: if `landscape_zoom_applies(...)` then `glib::timeout_add_local_once(500ms)` → anchor = Left/Right/Center per `forward` (behavior §9.6), target = `landscape_target_scale(view_h, src_h)`, animate 500 ms EASE_IN_OUT_QUAD, interruptible by any gesture (cancel the animation on user input).

## 7. Decoding and textures

- Decoder: `image` crate 0.25 with features `jpeg`, `png`, `webp`, `gif`, `bmp`, `tiff`. Decode into `RGBA8` (`image::DynamicImage::into_rgba8`). Animated GIF/WebP/APNG via `image::AnimationDecoder` → `AnimatedFrames`.
- Crop borders (`crop_borders`): scan each edge row/column for a uniform color within a tolerance; crop. Algorithm details and tolerance: decision in T-43b-020 (Kotlin uses SSIV fork `setCropBorders`, implementation in fork; behavior §9.3).
- Hardware threshold: `needs_tiling(w, h, DEFAULT_HW_THRESHOLD)`. Phase 1 (P0): if `max(w,h) > 16384` (GPU texture safety cap, proposal) downscale with `image::imageops::FilterType::Triangle`; otherwise upload full texture. Phase 2 (P1, T-43b-021): tiles of `threshold` px with min 180 dpi equivalent. Kotlin's `setMinimumTileDpi(180)` has no GTK equivalent; the tile selection rule is replaced by "tile when `needs_tiling` and the zoom would show more than one tile".
- Texture creation: `gdk::MemoryTexture::new(w as i32, h as i32, gdk::MemoryFormat::R8g8b8a8, &bytes, (w * 4) as usize)`. Use `glib::Bytes::from_owned(vec)` to avoid a copy.
- Caching: LRU of `gdk::Texture` for the current window ±N pages (N = 2). Drop on `recycle()`.
- Scale factor: textures are uploaded at native size; `snapshot` handles HiDPI scaling. Do not pre-scale for `scale_factor`.

## 8. Navigation, gestures and input

- Tap zones: `nav::action_at` is called with normalized coordinates `(x / width, y / height)` of the `view-clicked` event (the layout definitions live in area 44).
- Inversion: `TappingInvertMode` (pager and webtoon separately) → `(invert_h, invert_v)`.
- Keyboard: `Viewer::handle_key`. Proposed defaults (new, not in Aniyomi; documented as Proposal): Left/Right arrows and PageUp/PageDown page turn (direction swapped for RTL), Space forward, Shift+Space back, Home/End first/last page, `+`/`-` zoom, `0` reset zoom, `Ctrl+0` fit. These are additive; they do not change Aniyomi semantics.
- Volume keys (`reader_volume_keys`, `_inverted`): GTK does not reliably receive `XF86AudioRaiseVolume` / `XF86AudioLowerVolume` on KDE Plasma (the compositor/media keys often consume them). Proposal: map `XF86AudioRaiseVolume` → previous page (inverted: next) and `XF86AudioLowerVolume` → next (inverted: previous) when the pref is on; document that some desktops swallow these keys (T-43b-041).
- Mouse wheel and touchpad: `EventControllerScroll` in `Viewer::handle_scroll`. Kotlin `dispatchGenericMotionEvent` → `handleGenericMotionEvent`; GTK equivalent is scroll and pointer events on the viewer widget.
- Window: `fullscreen` pref → `window.fullscreen()`/`unfullscreen()`; `keep_screen_on` → `gtk::Application::inhibit(&window, gtk::ApplicationInhibitFlags::IDLE, Some("Reading"))` held while the reader is open and the pref is true; cookie removed on close (`uninhibit`).
- Cutout: no Linux equivalent (P3 drop).

## 9. Overlays

- `LoadingIndicator`: `gtk::Spinner` (always spinning, matches "always rotates") centered over the page area; optional percent label. Shown/hidden by part 1 ReaderActivity equivalent.
- `ChapterTransitionView`: `gtk::Box` (vertical) with labels for "Previous:", "Current:", "Finished:", "Next:" (keys from spec) and the downloaded status; `bind` recomputes the two booleans (behavior §11.2). Typography: `.caption`/`.body` style classes from the user's GTK theme (no hard-coded colors; match the Tokyo Night theme via the system CSS).
- `FlashHost`: `gtk::DrawingArea` overlay that draws a full-size rectangle with `cairo` or `snapshot.append_color` when `current_color.is_some()`; timers `glib::timeout_add_local_once(duration/2)`.
- `overlay_button`: `gtk::Button`. The pager-coupling workaround (`ReaderButton`) is not needed because GTK delivers button clicks to the button and not to the viewer's gesture controllers when the button is in the overlay (the viewer's gestures use `PropagationPhase::Bubble` and `GestureClick` claims are checked). Pitfall: verify with a test (T-43b-028).

## 10. Crates and why

| Need | Crate | Version / features | Why |
|---|---|---|---|
| GTK4 widgets, gestures, snapshot | `gtk` (package `gtk4`) | 0.11.5, `v4_16` (present) | mockup already uses it |
| Image decode | `image` | 0.25, features `jpeg,png,webp,gif,bmp,tiff` (default-features off) | pure Rust, animated GIF/WebP decode |
| Async/blocking pool for decode | `tokio` | 1.x, `rt-multi-thread`, `sync`, `macros` | decode off the GTK thread; shared runtime (other areas also need it) |
| SQLite kv store | `rusqlite` | `bundled` | DB shared with the rest of Kurobayomi (area 03/05) |
| Serialization of stored enums/keys | none (string encoding §3.1) | — | no serde needed for kv text |
| Error types | `thiserror` | 2.x | typed `DecodeError`, `PrefError` |
| Tests | built-in `cargo test` | — | pure modules |

No new crate for gestures (GTK provides them), timers (glib), or easing (hand-rolled 3 curves: quad ease-out and ease-in-out; formulas in T-43b-016).

## 11. Error handling

- `DecodeError { UnsupportedFormat(String), Corrupt(String), TooLarge { w: u32, h: u32 }, Io(std::io::Error) }`. Kotlin error path = `onImageLoadError`; Rust sets `LoadState::Error` and emits `image-load-error`. The page shows the error state (retry button) in the viewer (area 44; retry button = `overlay_button`).
- `PrefError` only for SQLite write failures: logged (`tracing` not in Cargo; use `eprintln!`-style logging via `log` crate? Proposal: `log` + `env_logger` in main; decided in area 01). Prefs keep the in-memory value; the next start may revert (documented).
- Invalid stored values: default + delete row (mirrors `ClassCastException` path).
- No `unwrap()` on decode or widget paths; the Kotlin `!!` sites (landscape zoom, pan) become early returns.

## 12. Android API to Linux mapping

| Android / Kotlin | Linux / Rust replacement | Notes |
|---|---|---|
| `SubsamplingScaleImageView` (tachiyomi fork, 66e0db195d) | `PageView` widget + `page_zoom` model | Fork behaviors reproduced: scale types, zoom start, double-tap focus, pan limit inside, landscape auto-zoom. Tiling = P1 |
| `PhotoView` 2.3.0 (animated) | same `PageView` with `AnimatedFrames` | scale levels 1,2,5 as absolute |
| `AppCompatImageView` (webtoon animated) | `PageView` with `is_webtoon` flag | no zoom |
| `WebtoonSubsamplingImageView` (ignores touch) | `PageView` with gestures disabled when `is_webtoon` | the webtoon viewer owns gestures |
| Coil (`ImageRequest`, `ViewSizeResolver`, `CachePolicy.DISABLED`, `customDecoder`) | `image` decode in `spawn_blocking` + `Vec` in memory | no disk cache, memory only for window |
| `ImageUtil.canUseHardwareBitmap` / `setHardwareConfig` | not applicable (Drop, T-43b-051) | GPU textures via gdk, no hardware bitmap flag |
| `BitmapRegionDecoder` | not applicable (Drop) | tiling is by Rust crop when needed |
| `Handler(Looper.getMainLooper())` + `postDelayed` | `glib::timeout_add_local_once` | same thread |
| `ViewConfiguration.getLongPressTimeout()` | `gtk::Settings::gtk-long-press-time` | runtime setting |
| `ViewConfiguration.getDoubleTapTimeout()` | `gtk::Settings::gtk-double-click-time` | runtime setting |
| `ViewConfiguration.scaledTouchSlop` | fixed 8 px (or `gtk-dnd-drag-threshold`) | Proposal |
| `Settings.Global.ANIMATOR_DURATION_SCALE` | `gtk::Settings::gtk-enable-animations` (bool) → factor 1.0 or 0.0 (then duration min 1 ms) | exact analogue is the GTK bool; no float scale on GTK |
| `AbstractComposeView` + `CombinedCircularProgressIndicator` | `gtk::Spinner` | always rotates |
| `MaterialButton` | `gtk::Button` | — |
| `BlendMode` (Compose/Android) | `gsk::BlendMode` (`Default`, `Multiply`, `Screen`, `Overlay`, `Lighten`, `Darken`) | SrcOver → Default. Applied via `snapshot.push_blend` around the page texture |
| `Canvas.drawRect` (flash) | `snapshot.append_color` in `FlashHost` draw | — |
| `ColorFilter` + brightness overlay | `snapshot.push_color_matrix` / `append_color` overlays | brightness -75..-1 → black overlay alpha; 1..100 → window brightness not controllable on Linux (Drop the 1..100 path, see T-43b-049) |
| `WindowManager.LayoutParams` cutout | none (P3 drop) | — |
| `FLAG_KEEP_SCREEN_ON` | `gtk::Application::inhibit(IDLE)` | — |
| `ActivityInfo.SCREEN_ORIENTATION_*` (part 1) | not applicable; rotation pref kept, meaning becomes "rotate display via compositor" → no-op on desktop (P3 for orientation lock) | — |
| `KeyEvent` volume keys | `EventControllerKey` + XF86 keysyms | desktop may swallow |
| `MotionEvent` generic motion | `EventControllerScroll` / motion controller | — |
| `Injekt.get<ReaderPreferences>()` | `Rc<ReaderPrefs>` passed explicitly | no DI container |
| `Flow.register` (ViewerConfig) | `Pref::subscribe` | §5 |
| `Preference.stateIn(scope)` | `Pref` + `glib` property | no coroutines |

## 13. Pitfalls

1. GTK 4 `gdk::Texture` creation on the wrong thread: create textures only on the GTK thread; the decode job returns bytes only.
2. `gdk::MemoryTexture` with `stride` must be `w*4`; a wrong stride shows diagonal garbage.
3. Large images: a 5000x7000 RGBA page is ~140 MB; keep the texture window to 3 pages and drop textures on `recycle()`.
4. `spawn_blocking` results arriving after the reader closed: check a session token before touching widgets.
5. `GestureDrag` with zoom == min must not steal page swipes (pass-through).
6. Pinch and drag gestures in GTK fire `begin`/`update`/`end` with offsets relative to the start; store start `zoom` and `center` at `begin`.
7. Double-click detection: GTK `GestureClick` `n_press` counts depend on `gtk-double-click-time`; the long-press tracker uses its own guard (behavior §8). Keep both; do not reuse GTK double-click for long press.
8. Animations: `gtk-enable-animations = false` must still complete the state change (instant), not skip it (Kotlin uses duration min 1 ms).
9. `keep_screen_on` inhibit must be released on window close and when the pref turns off, else the session stays inhibited.
10. Fractional scaling (e.g. 125%): zoom math is in logical px; `set_image` must not depend on device pixel size except for tiling thresholds (use source px).
11. The settings sheet is spec-rendered (`src/ui.rs`); do not duplicate pref state in the JSON. Items must bind to `Pref<T>`.
12. Enum-name encoding must be exact (`WHITE_BLACK`, not `WhiteBlack`), else backups from Aniyomi decode to defaults.
13. `ImageScaleType` stored 2 is crop-to-fill ("Stretch" label); do not "fix" it to a stretch without a decision (E-17).

## 14. Integration points in the Kurobayomi mockup

| File | Current | Change in this area |
|---|---|---|
| `src/player.rs:49-149` `reader(title, chapter)` | static OSD over a placeholder `cover(420,600)` | replace the canvas with `ReaderView` (page view + overlays); keep OSD structure (back button → `pop()`, bookmark toggle, menu). Tap zones via `nav::action_at` on the page view |
| `src/player.rs:122` comment "center tap toggles menus, side taps turn pages" | comment only | becomes real behavior through `nav` regions (layout in area 44) |
| `src/ui.rs:221 item_row` / `src/settings.rs` | renders spec JSON items | reader settings items bind to `ReaderPrefs` (T-43b-031); add a `pref` key to the item type in `src/spec.rs` (`Item` enum) |
| `src/spec.rs:8-24` `Item` enum | spec item variants | add optional `pref` field (key string) for reader items |
| `src/spec.rs:126` file list | includes `spec/reader_player.json` | unchanged |
| `src/main.rs:54-58` | immersive names | unchanged; add `screen:reader` fullscreen inhibit hook |
| `spec/reader_player.json` | labels for reader sheet | keep labels; add `pref` keys to items per `p2-data-and-config.md` §2 |
| new `src/reader/` | — | modules in §2 |
| `Cargo.toml` | gtk, serde, serde_json | add `image`, `tokio`, `rusqlite` (bundled), `thiserror` (see §10) |

Compatibility: the `reader_player.json` labels for the reader sheet match Aniyomi's English strings closely (e.g. "Tap zones", "Invert tap zones", "Scale type", "Zoom start position"). Keep Aniyomi's wording where the key exists so translations (area 51) map directly.
