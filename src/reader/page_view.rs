//! The page view (T-43b-013, T-43b-014, T-43b-016, T-43b-024, T-44a-044, T-44a-049, T-44a-059). One page: the bytes decode
//! on the blocking pool into a pyramid of tiles, the tiles that the view shows become `gdk::Texture`s on the GTK thread,
//! and `snapshot` draws them at the zoom and centre that the pure model in `page_zoom` gives. Loading shows the indicator
//! and hides the page; a failed load emits `image-load-error`.
//!
//! Signals: `image-loaded`, `image-load-error`, `scale-changed` (f64), `view-clicked`, `long-tap` (f64, f64).
//! The long press is the tracker in `long_press`, armed by the GTK click gesture. It is reported only while
//! [`PageView::set_long_tap_enabled`] is on (`reader_long_tap`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::subclass::prelude::ObjectSubclassIsExt;
use gtk::{gdk, gio, glib, graphene};

use crate::reader::decode_guard::{self, Ticket};
use crate::reader::decoded_lru::{ByteLru, DEFAULT_BUDGET_BYTES};
use crate::reader::long_press::{DEFAULT_DOUBLE_CLICK_MS, DEFAULT_LONG_PRESS_MS, LongPressAction, LongPressTracker, SLOP_PX};
use crate::reader::page_decode::{self, DecodeError, LoadState};
use crate::reader::page_transform::{self, DecodeIntent};
use crate::reader::page_zoom::{self as zoom, PageConfig, Size, ZoomState};
use crate::reader::progress::LoadingIndicator;
use crate::reader::tiles::{MAX_TILE_EDGE, Pyramid, Tile};
use crate::reader::viewers::page_bytes::ByteSource;

/// The result of a decode: the pyramid of the page and whether a transform changed it, or why it failed.
type Decoded = std::thread::Result<Result<(Pyramid, bool), DecodeError>>;

/// Decodes `bytes`, applies the transform of `intent`, and builds the pyramid. A webtoon page is cut to `width` after
/// the transform (never enlarged). All of it runs on the worker; the texture is made later on the GTK thread.
fn decode_pyramid(bytes: &[u8], width: Option<u32>, intent: DecodeIntent) -> Result<(Pyramid, bool), DecodeError> {
    let page = page_decode::decode_rgba(bytes)?;
    let (page, changed) = page_transform::apply(page, intent);
    let page = match width {
        Some(width) => page_decode::fit_to_width(page, width),
        None => page,
    };
    Ok((Pyramid::new(page, MAX_TILE_EDGE), changed))
}

/// What identifies a decode in the cache: where the bytes come from, what was done to them, and the width they were cut
/// to (T-44a-069).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DecodeKey {
    source: ByteSource,
    intent: DecodeIntent,
    width: Option<u32>,
}

/// A cached decode. A page that shows it holds its own share of the pyramid, which is how the cache knows the entry is
/// in use.
#[derive(Clone)]
struct Cached {
    pyramid: Rc<Pyramid>,
    split: bool,
}

thread_local! {
    /// The decoded pages kept by byte budget. A page that leaves the realised window drops its widget but not its pixels.
    static DECODED: RefCell<ByteLru<DecodeKey, Cached>> = RefCell::new(ByteLru::new(DEFAULT_BUDGET_BYTES));
}

fn cache_get(key: &DecodeKey) -> Option<Cached> {
    DECODED.with(|c| c.borrow_mut().get(key))
}

fn cache_put(key: DecodeKey, cached: &Cached) {
    let bytes = cached.pyramid.byte_size();
    DECODED.with(|c| c.borrow_mut().insert(key, cached.clone(), bytes, |e| Rc::strong_count(&e.pyramid) > 1));
}

/// Forgets every decoded page. A new reader session starts with none.
pub fn clear_decoded_cache() {
    DECODED.with(|c| c.borrow_mut().clear());
}

/// Uploads one tile as a texture. `None` for a tile outside its level.
fn upload(pyramid: &Pyramid, tile: Tile) -> Option<gdk::Texture> {
    let bytes = pyramid.tile_bytes(tile);
    if bytes.is_empty() {
        return None;
    }
    let stride = tile.w as usize * 4;
    let bytes = glib::Bytes::from_owned(bytes);
    let texture = gdk::MemoryTexture::new(tile.w as i32, tile.h as i32, gdk::MemoryFormat::R8g8b8a8, &bytes, stride);
    Some(texture.upcast::<gdk::Texture>())
}

mod imp {
    use super::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct PageView {
        /// The decoded page as a pyramid of tiles (level 0 is the full page). Only the visible tiles are uploaded.
        pub pyramid: RefCell<Option<Rc<Pyramid>>>,
        /// Uploaded tiles. A tile that leaves the view is dropped on the next frame (T-44a-049).
        pub tiles: RefCell<HashMap<Tile, gdk::Texture>>,
        pub src: Cell<Size>,
        pub state: Cell<LoadState>,
        /// Bumped by every load, recycle and cancelled timer, so a late result or timer is dropped.
        pub generation: Cell<u64>,
        pub config: Cell<PageConfig>,
        /// The view size the zoom was last fitted to. `None` means the zoom resets to the fit and the anchor.
        pub fitted: Cell<Option<(i32, i32)>>,
        /// The zoom of the page: scale, centre, fitted minimum, maximum and double-tap level (T-44a-044).
        pub zoom: Cell<ZoomState>,
        /// Whether a decode is cut to the view width first (webtoon pages, B§8.4).
        pub decode_to_view: Cell<bool>,
        /// Set for a webtoon page: it does not zoom on its own, the strip does (T-44a-039).
        pub zoom_locked: Cell<bool>,
        pub background: RefCell<Option<gdk::RGBA>>,
        pub long: RefCell<Option<LongPressTracker>>,
        pub long_gen: Cell<u64>,
        pub long_fired: Cell<bool>,
        pub long_tap_enabled: Cell<bool>,
        pub press_at: Cell<(f64, f64)>,
        pub pointer: Cell<Option<(f64, f64)>>,
        pub pinch_start: Cell<f64>,
        pub drag_start: Cell<(f64, f64)>,
        pub indicator: RefCell<Option<LoadingIndicator>>,
        /// Bumped to stop a running zoom animation.
        pub anim_gen: Cell<u64>,
        pub double_tap_ms: Cell<u32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PageView {
        const NAME: &'static str = "KuroReaderPageView";
        type Type = super::PageView;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for PageView {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_hexpand(true);
            obj.set_vexpand(true);
            let indicator = LoadingIndicator::new();
            indicator.widget().set_parent(&*obj);
            *self.indicator.borrow_mut() = Some(indicator);
            *self.long.borrow_mut() = Some(long_press_tracker());
            self.install_controllers();
        }

        fn dispose(&self) {
            if let Some(indicator) = self.indicator.take() {
                indicator.widget().unparent();
            }
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: std::sync::OnceLock<Vec<glib::subclass::Signal>> = std::sync::OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    glib::subclass::Signal::builder("image-loaded").build(),
                    glib::subclass::Signal::builder("image-load-error").build(),
                    glib::subclass::Signal::builder("page-split").build(),
                    glib::subclass::Signal::builder("scale-changed").param_types([glib::Type::F64]).build(),
                    glib::subclass::Signal::builder("view-clicked").build(),
                    glib::subclass::Signal::builder("long-tap").param_types([glib::Type::F64, glib::Type::F64]).build(),
                ]
            })
        }
    }

    impl WidgetImpl for PageView {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let (w, h) = (f64::from(obj.width()), f64::from(obj.height()));
            if w > 0.0 && h > 0.0 {
                self.fit_to(Size { w, h });
            }
            if self.state.get() == LoadState::Ready {
                self.draw_page(snapshot, w, h);
            }
            if let Some(child) = obj.first_child() {
                obj.snapshot_child(&child, snapshot);
            }
        }
    }

    impl PageView {
        fn view(&self) -> Size {
            let obj = self.obj();
            Size { w: f64::from(obj.width()), h: f64::from(obj.height()) }
        }

        /// Draws the visible tiles at the zoom. Tiles that are not visible are released first (T-44a-049).
        fn draw_page(&self, snapshot: &gtk::Snapshot, w: f64, h: f64) {
            let pyramid = self.pyramid.borrow();
            let Some(pyramid) = pyramid.as_ref() else { return };
            let z = self.zoom.get();
            let bounds = graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
            snapshot.push_clip(&bounds);
            if let Some(background) = *self.background.borrow() {
                snapshot.append_color(&background, &bounds);
            }
            let (_, visible) = pyramid.visible_tiles(z.scale, z.center, (w, h));
            let mut uploaded = self.tiles.borrow_mut();
            uploaded.retain(|tile, _| visible.contains(tile));
            for tile in visible {
                if !uploaded.contains_key(&tile) {
                    if let Some(texture) = upload(pyramid, tile) {
                        uploaded.insert(tile, texture);
                    }
                }
                let Some(texture) = uploaded.get(&tile) else { continue };
                let (x, y, tw, th) = Pyramid::screen_rect(tile, z.scale, z.center, (w, h));
                snapshot.append_texture(texture, &graphene::Rect::new(x as f32, y as f32, tw as f32, th as f32));
            }
            snapshot.pop();
        }

        /// Fits the zoom to the view. A new page or a reset starts at the fit and the zoom-start anchor. A resize keeps
        /// a fitted page fitted and otherwise keeps the zoom, clamped to the new range.
        fn fit_to(&self, view: Size) {
            if self.state.get() != LoadState::Ready {
                return;
            }
            let key = (view.w as i32, view.h as i32);
            let src = self.src.get();
            let config = self.config.get();
            let min = zoom::min_scale(config.scale, view, src);
            let before = self.zoom.get();
            let mut z = match self.fitted.get() {
                Some(previous) if previous == key => return,
                Some(_) => {
                    let scale = if before.is_fitted() { min } else { before.scale };
                    let kept = ZoomState { scale, center: before.center, min, max: min * zoom::MAX_ZOOM_FACTOR, double_tap: zoom::double_tap_level(min) };
                    ZoomState { scale: kept.clamp(kept.scale), ..kept }
                }
                None => ZoomState::fitted(min, zoom::anchor_center(config.zoom_start, src)),
            };
            z.center = zoom::clamp_center(z.center, view, src, z.scale);
            self.zoom.set(z);
            self.fitted.set(Some(key));
            if (z.scale - before.scale).abs() > 1e-9 {
                // Every scale change reports, the first fit included (behaviour section 9.1). Deferred, because this
                // runs inside a snapshot.
                let weak = self.obj().downgrade();
                glib::idle_add_local_once(move || {
                    on_page(&weak, |p| {
                        let scale = p.zoom.get().scale;
                        p.obj().emit_by_name::<()>("scale-changed", &[&scale]);
                    });
                });
            }
        }

        pub(super) fn set_state(&self, state: LoadState) {
            self.state.set(state);
            if let Some(indicator) = self.indicator.borrow().as_ref() {
                if state == LoadState::Loading {
                    indicator.show();
                } else {
                    indicator.hide();
                }
            }
            self.obj().queue_draw();
        }

        /// Drops the pyramid and every uploaded tile.
        pub(super) fn drop_pages(&self) {
            self.pyramid.replace(None);
            self.tiles.borrow_mut().clear();
        }

        /// Finishes a decode started by `set_image`. A result whose ticket is no longer current (another session, or
        /// the page was loaded again) is dropped without touching anything (E-40, T-43b-059). A good result goes to the
        /// decoded cache under `key` before the page shows it.
        pub(super) fn finish_load(&self, ticket: Ticket, decoded: Decoded, key: Option<DecodeKey>) {
            if !decode_guard::accepts(ticket, decode_guard::current_session(), self.generation.get()) {
                return;
            }
            match decoded {
                Ok(Ok((pyramid, split))) if pyramid.size().0 > 0 && pyramid.size().1 > 0 => {
                    let cached = Cached { pyramid: Rc::new(pyramid), split };
                    if let Some(key) = key {
                        cache_put(key, &cached);
                    }
                    self.show_pyramid(cached);
                }
                _ => {
                    self.drop_pages();
                    self.set_state(LoadState::Error);
                    self.obj().emit_by_name::<()>("image-load-error", &[]);
                }
            }
        }

        /// Shows a decoded page: sizes the zoom to it and reports `image-loaded`, and `page-split` when a transform
        /// split it.
        pub(super) fn show_pyramid(&self, cached: Cached) {
            let (w, h) = cached.pyramid.size();
            self.src.set(Size { w: f64::from(w), h: f64::from(h) });
            self.pyramid.replace(Some(cached.pyramid));
            self.tiles.borrow_mut().clear();
            self.fitted.set(None);
            self.set_state(LoadState::Ready);
            let obj = self.obj();
            obj.emit_by_name::<()>("image-loaded", &[]);
            if cached.split {
                obj.emit_by_name::<()>("page-split", &[]);
            }
        }

        fn install_controllers(&self) {
            let obj = self.obj();

            let click = gtk::GestureClick::new();
            let weak = obj.downgrade();
            click.connect_pressed(move |g, n, x, y| {
                let t = g.current_event_time();
                on_page(&weak, |p| p.press(n, x, y, t));
            });
            let weak = obj.downgrade();
            click.connect_released(move |g, n, x, y| {
                let t = g.current_event_time();
                on_page(&weak, |p| p.release(n, x, y, t));
            });
            let weak = obj.downgrade();
            click.connect_cancel(move |_, _| on_page(&weak, |p| p.cancel()));
            obj.add_controller(click);

            // a right-click opens the page actions like a long press does (T-43a-045); it is deliberate, so it does
            // not wait for the long-press setting
            let secondary = gtk::GestureClick::builder().button(gdk::BUTTON_SECONDARY).build();
            let weak = obj.downgrade();
            secondary.connect_pressed(move |_, _, x, y| {
                on_page(&weak, |p| p.obj().emit_by_name::<()>("long-tap", &[&x, &y]));
            });
            obj.add_controller(secondary);

            let motion = gtk::EventControllerMotion::new();
            let weak = obj.downgrade();
            motion.connect_motion(move |_, x, y| on_page(&weak, |p| p.motion(x, y)));
            obj.add_controller(motion);

            let drag = gtk::GestureDrag::new();
            let weak = obj.downgrade();
            drag.connect_drag_begin(move |_, _, _| on_page(&weak, |p| p.drag_begin()));
            let weak = obj.downgrade();
            drag.connect_drag_update(move |_, dx, dy| on_page(&weak, |p| p.drag_update(dx, dy)));
            obj.add_controller(drag);

            let pinch = gtk::GestureZoom::new();
            let weak = obj.downgrade();
            pinch.connect_begin(move |_, _| on_page(&weak, |p| p.pinch_begin()));
            let weak = obj.downgrade();
            pinch.connect_scale_changed(move |_, factor| on_page(&weak, |p| p.pinch(factor)));
            obj.add_controller(pinch);

            let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
            let weak = obj.downgrade();
            scroll.connect_scroll(move |c, _, dy| {
                if !c.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) {
                    return glib::Propagation::Proceed;
                }
                on_page(&weak, |p| p.wheel(dy < 0.0));
                glib::Propagation::Stop
            });
            obj.add_controller(scroll);
        }

        pub(super) fn press(&self, n: i32, x: f64, y: f64, t: u32) {
            self.press_at.set((x, y));
            self.pointer.set(Some((x, y)));
            if n >= 2 {
                // The second tap of a double tap: no long press, and the pending timer (if any) is dropped.
                self.long_action(|tr| tr.on_cancel());
                self.bump_long();
                if !self.zoom_locked.get() {
                    self.double_tap(x, y);
                }
                return;
            }
            self.long_fired.set(false);
            if let LongPressAction::Arm { after_ms } = self.long_action(|tr| tr.on_press(t, x, y)) {
                self.arm(after_ms);
            }
        }

        fn release(&self, n: i32, x: f64, y: f64, t: u32) {
            self.long_action(|tr| tr.on_release(t));
            self.bump_long();
            let (px, py) = self.press_at.get();
            let moved = (x - px).abs() > SLOP_PX || (y - py).abs() > SLOP_PX;
            if n == 1 && !self.long_fired.get() && !moved {
                self.obj().emit_by_name::<()>("view-clicked", &[]);
            }
        }

        fn cancel(&self) {
            self.long_action(|tr| tr.on_cancel());
            self.bump_long();
        }

        fn motion(&self, x: f64, y: f64) {
            self.pointer.set(Some((x, y)));
            if self.long_action(|tr| tr.on_motion(x, y)) == LongPressAction::Disarm {
                self.bump_long();
            }
        }

        fn long_action(&self, f: impl FnOnce(&mut LongPressTracker) -> LongPressAction) -> LongPressAction {
            self.long.borrow_mut().as_mut().map_or(LongPressAction::None, f)
        }

        fn bump_long(&self) {
            self.long_gen.set(self.long_gen.get() + 1);
        }

        fn arm(&self, after_ms: u32) {
            let generation = self.long_gen.get() + 1;
            self.long_gen.set(generation);
            let weak = self.obj().downgrade();
            glib::timeout_add_local_once(Duration::from_millis(u64::from(after_ms)), move || {
                on_page(&weak, |p| p.long_timer(generation));
            });
        }

        fn long_timer(&self, generation: u64) {
            if self.long_gen.get() != generation {
                return;
            }
            let confirmed = self.long.borrow_mut().as_mut().is_some_and(|tr| tr.fire());
            if !confirmed {
                return;
            }
            self.long_fired.set(true);
            if self.long_tap_enabled.get() {
                let (x, y) = self.press_at.get();
                self.obj().emit_by_name::<()>("long-tap", &[&x, &y]);
            }
        }

        fn double_tap(&self, x: f64, y: f64) {
            if self.state.get() != LoadState::Ready {
                return;
            }
            let z = self.zoom.get();
            let target = zoom::double_tap_target(z.scale, z.min);
            let enabled = gtk::Settings::default().is_none_or(|s| s.is_gtk_enable_animations());
            self.animate_zoom(target, (x, y), zoom::animation_ms(self.double_tap_ms.get(), enabled));
        }

        fn wheel(&self, up: bool) {
            if self.state.get() != LoadState::Ready || self.zoom_locked.get() {
                return;
            }
            let focus = self.pointer.get().unwrap_or_else(|| {
                let view = self.view();
                (view.w / 2.0, view.h / 2.0)
            });
            let z = self.zoom.get();
            let target = zoom::wheel_zoom(z.scale, z.min, up);
            self.set_zoom(target, focus);
        }

        pub(super) fn pinch_begin(&self) {
            if self.zoom_locked.get() {
                return;
            }
            self.stop_animation();
            self.pinch_start.set(self.zoom.get().scale);
            self.cancel();
        }

        pub(super) fn pinch(&self, factor: f64) {
            if self.state.get() != LoadState::Ready || self.zoom_locked.get() {
                return;
            }
            let view = self.view();
            self.set_zoom(zoom::pinch_scale(self.pinch_start.get(), factor, &self.zoom.get()), (view.w / 2.0, view.h / 2.0));
        }

        fn drag_begin(&self) {
            if self.zoom_locked.get() {
                return;
            }
            self.stop_animation();
            self.drag_start.set(self.zoom.get().center);
        }

        fn drag_update(&self, dx: f64, dy: f64) {
            let mut z = self.zoom.get();
            if self.state.get() != LoadState::Ready || z.scale <= 0.0 || self.zoom_locked.get() {
                return;
            }
            let (sx, sy) = self.drag_start.get();
            let view = self.view();
            z.center = zoom::clamp_center((sx - dx / z.scale, sy - dy / z.scale), view, self.src.get(), z.scale);
            self.zoom.set(z);
            self.obj().queue_draw();
        }

        pub(super) fn stop_animation(&self) {
            self.anim_gen.set(self.anim_gen.get() + 1);
        }

        /// Zooms to `target` over `duration_ms` (T-43b-016), the source point under `focus` staying put. A duration of
        /// 1 ms or less zooms at once.
        pub(super) fn animate_zoom(&self, target: f64, focus: (f64, f64), duration_ms: u32) {
            if duration_ms <= 1 {
                self.set_zoom(target, focus);
                return;
            }
            self.stop_animation();
            let generation = self.anim_gen.get();
            let (view, src) = (self.view(), self.src.get());
            let start = self.zoom.get();
            let (from, begin) = (start.scale, start.center);
            let to = start.clamp(target);
            let began = std::time::Instant::now();
            let weak = self.obj().downgrade();
            glib::timeout_add_local(Duration::from_millis(16), move || {
                let Some(page) = weak.upgrade() else { return glib::ControlFlow::Break };
                let p = page.imp();
                if p.anim_gen.get() != generation {
                    return glib::ControlFlow::Break;
                }
                let elapsed = began.elapsed().as_secs_f64() * 1000.0;
                let done = elapsed >= f64::from(duration_ms);
                let scale = zoom::zoom_step(from, to, elapsed, duration_ms);
                let center = zoom::clamp_center(zoom::zoom_about(begin, view, from, scale, focus), view, src, scale);
                p.zoom.set(ZoomState { scale, center, ..p.zoom.get() });
                page.queue_draw();
                page.emit_by_name::<()>("scale-changed", &[&scale]);
                if done { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
            });
        }

        /// Sets the zoom to `target` (clamped to the page's range) while the source point under `focus` stays put.
        fn set_zoom(&self, target: f64, focus: (f64, f64)) {
            self.stop_animation();
            let view = self.view();
            let src = self.src.get();
            let z = self.zoom.get();
            let new = z.clamp(target);
            let center = zoom::zoom_about(z.center, view, z.scale, new, focus);
            self.zoom.set(ZoomState { scale: new, center: zoom::clamp_center(center, view, src, new), ..z });
            let obj = self.obj();
            obj.queue_draw();
            obj.emit_by_name::<()>("scale-changed", &[&new]);
        }
    }

    fn long_press_tracker() -> LongPressTracker {
        let settings = gtk::Settings::default();
        let long_ms = settings.as_ref().and_then(|s| u32::try_from(s.gtk_long_press_time()).ok()).unwrap_or(DEFAULT_LONG_PRESS_MS);
        let double_ms = settings.as_ref().and_then(|s| u32::try_from(s.gtk_double_click_time()).ok()).unwrap_or(DEFAULT_DOUBLE_CLICK_MS);
        LongPressTracker::new(long_ms.max(1), double_ms)
    }

    /// Runs `f` on the page if it is still alive. Signal handlers hold weak references so that a page closed while a
    /// timer or decode is pending is not kept alive by them.
    pub(super) fn on_page(weak: &glib::WeakRef<super::PageView>, f: impl FnOnce(&PageView)) {
        if let Some(page) = weak.upgrade() {
            f(page.imp());
        }
    }
}

glib::wrapper! {
    pub struct PageView(ObjectSubclass<imp::PageView>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PageView {
    fn default() -> Self {
        Self::new()
    }
}

impl PageView {
    pub fn new() -> Self {
        glib::Object::new()
    }

    /// Starts decoding `bytes` for this page. A page still loading is replaced, and its result is dropped. The decode
    /// belongs to the current reader session; a result from any other session is dropped (T-43b-059).
    pub fn set_image(&self, bytes: Vec<u8>, config: PageConfig) {
        self.load(bytes, config, DecodeIntent::NONE, None);
    }

    /// Like [`set_image`], with the transform to apply on the worker. The decoded page is kept in the byte-budget cache
    /// under `source` and `intent`, so [`show_cached`] finds it again (T-44a-069).
    pub fn set_image_from(&self, source: ByteSource, bytes: Vec<u8>, config: PageConfig, intent: DecodeIntent) {
        let key = self.cache_key(source, intent);
        self.load(bytes, config, intent, Some(key));
    }

    /// Whether the cache key of this page is settled: a webtoon page is keyed by the view width, which is 0 until the
    /// page has been allocated.
    pub fn cache_key_ready(&self) -> bool {
        !self.imp().decode_to_view.get() || self.width() > 0
    }

    /// Shows the page from the decoded cache when it is there, with no decode. `false` when it is not.
    pub fn show_cached(&self, source: ByteSource, config: PageConfig, intent: DecodeIntent) -> bool {
        let Some(cached) = cache_get(&self.cache_key(source, intent)) else { return false };
        let imp = self.imp();
        imp.stop_animation();
        imp.config.set(config);
        // a decode still running for this page is stale now
        imp.generation.set(imp.generation.get() + 1);
        imp.show_pyramid(cached);
        true
    }

    fn cache_key(&self, source: ByteSource, intent: DecodeIntent) -> DecodeKey {
        let width = self.imp().decode_to_view.get().then(|| self.width().max(0) as u32).filter(|w| *w > 0);
        DecodeKey { source, intent, width }
    }

    fn load(&self, bytes: Vec<u8>, config: PageConfig, intent: DecodeIntent, key: Option<DecodeKey>) {
        let imp = self.imp();
        imp.stop_animation();
        imp.config.set(config);
        imp.fitted.set(None);
        imp.drop_pages();
        let generation = imp.generation.get() + 1;
        imp.generation.set(generation);
        imp.set_state(LoadState::Loading);
        let ticket = Ticket::now(generation);
        let width = key.as_ref().map_or_else(|| imp.decode_to_view.get().then(|| self.width().max(0) as u32).filter(|w| *w > 0), |k| k.width);
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let decoded = gio::spawn_blocking(move || decode_pyramid(&bytes, width, intent)).await;
            imp::on_page(&weak, |p| p.finish_load(ticket, decoded, key));
        });
    }

    /// Drops the pyramid and any pending decode, and shows nothing until the next `set_image`.
    pub fn recycle(&self) {
        let imp = self.imp();
        imp.stop_animation();
        imp.generation.set(imp.generation.get() + 1);
        imp.drop_pages();
        imp.fitted.set(None);
        imp.set_state(LoadState::Empty);
    }

    /// Applies new display settings to the shown page without reloading its texture (D-11).
    pub fn set_config(&self, config: PageConfig) {
        let imp = self.imp();
        imp.stop_animation();
        imp.config.set(config);
        imp.fitted.set(None);
        self.queue_draw();
    }

    /// The double-tap zoom duration in ms (`pref_double_tap_anim_speed`), before the GTK animation setting scales it.
    pub fn set_double_tap_anim_ms(&self, ms: u32) {
        self.imp().double_tap_ms.set(ms);
    }

    /// Whether the page zooms on its own. A webtoon page does not: the strip does (T-44a-039).
    pub fn set_zoomable(&self, zoomable: bool) {
        self.imp().zoom_locked.set(!zoomable);
    }

    /// Cuts decodes to the view width first, as a webtoon page does (B§8.4).
    pub fn set_decode_to_view(&self, enabled: bool) {
        self.imp().decode_to_view.set(enabled);
    }

    /// Whether `long-tap` reports (`reader_long_tap`).
    pub fn long_tap_enabled(&self) -> bool {
        self.imp().long_tap_enabled.get()
    }

    pub fn state(&self) -> LoadState {
        self.imp().state.get()
    }

    pub fn scale(&self) -> f64 {
        self.imp().zoom.get().scale
    }

    /// The page background, drawn only once a page has loaded.
    pub fn set_page_background(&self, colour: Option<gdk::RGBA>) {
        self.imp().background.replace(colour);
        self.queue_draw();
    }

    /// Whether a confirmed long press reports `long-tap` (`reader_long_tap`).
    pub fn set_long_tap_enabled(&self, enabled: bool) {
        self.imp().long_tap_enabled.set(enabled);
    }

    pub fn connect_image_loaded<F: Fn() + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("image-loaded", false, move |_| {
            f();
            None
        })
    }

    pub fn connect_image_load_error<F: Fn() + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("image-load-error", false, move |_| {
            f();
            None
        })
    }

    pub fn connect_scale_changed<F: Fn(f64) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("scale-changed", false, move |values| {
            f(values[1].get::<f64>().unwrap_or_default());
            None
        })
    }

    /// Reports that a transform split the page in two, so the viewer places its insert page.
    pub fn connect_page_split<F: Fn() + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("page-split", false, move |_| {
            f();
            None
        })
    }

    pub fn connect_view_clicked<F: Fn() + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("view-clicked", false, move |_| {
            f();
            None
        })
    }

    /// Reports a confirmed long press with its press position.
    pub fn connect_long_tap<F: Fn(f64, f64) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("long-tap", false, move |values| {
            f(values[1].get::<f64>().unwrap_or_default(), values[2].get::<f64>().unwrap_or_default());
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).expect("encode");
        out.into_inner()
    }

    /// Runs the main context for a while, so frames are drawn and fitted.
    fn pump(ctx: &glib::MainContext) {
        for _ in 0..100 {
            while ctx.iteration(false) {}
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Runs the main context until the page leaves `Loading`, or gives up after a few seconds.
    fn settle(page: &PageView) {
        let ctx = glib::MainContext::default();
        for _ in 0..2000 {
            if page.state() != LoadState::Loading {
                return;
            }
            ctx.iteration(false);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn a_page_loads_and_recycle_drops_it_then_a_new_one_loads() {
        crate::testing::with_display(|| {
            let page = PageView::new();
            assert_eq!(page.state(), LoadState::Empty);
            page.set_image(png(3, 2), PageConfig::default());
            assert_eq!(page.state(), LoadState::Loading);
            settle(&page);
            assert_eq!(page.state(), LoadState::Ready);
            page.recycle();
            assert_eq!(page.state(), LoadState::Empty);
            page.set_image(png(4, 4), PageConfig::default());
            settle(&page);
            assert_eq!(page.state(), LoadState::Ready);
        });
    }

    #[test]
    fn a_corrupt_page_enters_the_error_state_and_emits_the_signal() {
        crate::testing::with_display(|| {
            let page = PageView::new();
            let failed = std::rc::Rc::new(Cell::new(false));
            let seen = std::rc::Rc::clone(&failed);
            page.connect_image_load_error(move || seen.set(true));
            page.set_image(b"not an image".to_vec(), PageConfig::default());
            settle(&page);
            assert_eq!(page.state(), LoadState::Error);
            assert!(failed.get());
        });
    }

    #[test]
    fn e01_a_tall_page_is_fitted_to_the_view_when_shown() {
        crate::testing::with_display(|| {
            let page = PageView::new();
            let window = gtk::Window::new();
            window.set_default_size(800, 600);
            window.set_child(Some(&page));
            window.present();
            page.set_image(png(1000, 2000), PageConfig::default());
            settle(&page);
            let ctx = glib::MainContext::default();
            pump(&ctx);
            // FitScreen on a 1000x2000 page in an 800x600 view: min(0.8, 0.3) = 0.3.
            assert!((page.scale() - 0.3).abs() < 0.01, "scale {}", page.scale());
            window.destroy();
        });
    }

    #[test]
    fn e01_a_wide_page_is_fitted_to_the_view_when_shown() {
        crate::testing::with_display(|| {
            let page = PageView::new();
            let window = gtk::Window::new();
            window.set_default_size(800, 600);
            window.set_child(Some(&page));
            let reported = std::rc::Rc::new(Cell::new(0.0));
            let seen = std::rc::Rc::clone(&reported);
            page.connect_scale_changed(move |s| seen.set(s));
            window.present();
            page.set_image(png(2000, 1000), PageConfig::default());
            settle(&page);
            let ctx = glib::MainContext::default();
            pump(&ctx);
            // FitScreen on a 2000x1000 page in an 800x600 view: min(0.4, 0.6) = 0.4, reported by the initial fit.
            assert!((page.scale() - 0.4).abs() < 0.01, "scale {}", page.scale());
            assert!((reported.get() - 0.4).abs() < 0.01, "scale-changed reported {}", reported.get());
            window.destroy();
        });
    }

    #[test]
    fn changing_the_config_refits_without_reloading_the_texture() {
        crate::testing::with_display(|| {
            let page = PageView::new();
            let window = gtk::Window::new();
            window.set_default_size(800, 600);
            window.set_child(Some(&page));
            window.present();
            page.set_image(png(2000, 1000), PageConfig::default());
            settle(&page);
            let ctx = glib::MainContext::default();
            pump(&ctx);
            page.set_config(PageConfig { scale: zoom::ImageScaleType::Stretch, ..PageConfig::default() });
            pump(&ctx);
            assert_eq!(page.state(), LoadState::Ready, "no reload");
            assert!((page.scale() - 0.6).abs() < 0.01, "stretch fills the larger fit: {}", page.scale());
            page.set_config(PageConfig { scale: zoom::ImageScaleType::FitWidth, ..PageConfig::default() });
            pump(&ctx);
            assert!((page.scale() - 0.4).abs() < 0.01, "fit width of 2000 in 800: {}", page.scale());
            window.destroy();
        });
    }

    #[test]
    fn a_result_from_an_old_generation_or_session_is_not_applied() {
        // The guard that finish_load uses: a ticket applies only to the session and generation it was started for.
        let ticket = Ticket { session: 1, generation: 2 };
        assert!(decode_guard::accepts(ticket, 1, 2));
        assert!(!decode_guard::accepts(ticket, 1, 3));
        assert!(!decode_guard::accepts(ticket, 2, 2));
    }

    fn wide_png() -> Vec<u8> {
        png(400, 200)
    }

    fn settle_all() {
        pump(&glib::MainContext::default());
    }

    fn file(name: &str) -> ByteSource {
        ByteSource::File(std::path::PathBuf::from(format!("/page-view-tests/{name}")))
    }

    #[test]
    fn a_decoded_page_is_kept_in_the_cache_and_a_second_view_shows_it_without_decoding() {
        crate::testing::with_display(|| {
            let (source, config) = (file("cached.png"), PageConfig::default());
            let first = PageView::new();
            assert!(!first.show_cached(source.clone(), config, DecodeIntent::NONE), "nothing is cached yet");
            first.set_image_from(source.clone(), png(30, 20), config, DecodeIntent::NONE);
            settle(&first);
            assert_eq!(first.state(), LoadState::Ready);
            let second = PageView::new();
            assert!(second.show_cached(source.clone(), config, DecodeIntent::NONE));
            assert_eq!(second.state(), LoadState::Ready, "shown at once, with no decode");
            assert_eq!(second.imp().src.get(), Size { w: 30.0, h: 20.0 });
            let rotated = DecodeIntent { transform: page_transform::PageTransform::RotateWide { invert: false }, only_if_wide: false };
            assert!(!PageView::new().show_cached(source, config, rotated), "another transform is another entry");
        });
    }

    #[test]
    fn a_view_holds_a_share_of_its_cached_pyramid_which_is_what_pins_it_against_eviction() {
        crate::testing::with_display(|| {
            let view = PageView::new();
            view.set_image_from(file("pinned.png"), png(8, 8), PageConfig::default(), DecodeIntent::NONE);
            settle(&view);
            let key = DecodeKey { source: file("pinned.png"), intent: DecodeIntent::NONE, width: None };
            let cached = cache_get(&key).expect("cached");
            assert_eq!(Rc::strong_count(&cached.pyramid), 3, "the cache, the view and this handle");
            view.recycle();
            assert_eq!(Rc::strong_count(&cached.pyramid), 2, "the view let go; the pixels stay cached for the way back");
        });
    }

    #[test]
    fn the_transform_is_applied_on_the_worker_and_a_split_is_reported() {
        crate::testing::with_display(|| {
            let page = PageView::new();
            let splits = Rc::new(Cell::new(0));
            let seen = Rc::clone(&splits);
            page.connect_page_split(move || seen.set(seen.get() + 1));
            let split = DecodeIntent { transform: page_transform::PageTransform::SplitLeft, only_if_wide: true };
            page.set_image_from(file("wide.png"), wide_png(), PageConfig::default(), split);
            settle(&page);
            assert_eq!(page.imp().src.get(), Size { w: 200.0, h: 200.0 }, "the left half of a 400 by 200 page");
            assert_eq!(splits.get(), 1);
            // a tall page is not wide: it is kept whole and nothing is reported
            page.set_image_from(file("tall.png"), png(100, 300), PageConfig::default(), split);
            settle(&page);
            assert_eq!(page.imp().src.get(), Size { w: 100.0, h: 300.0 });
            assert_eq!(splits.get(), 1);
            // the cached page reports its split again for a row that is built later
            let again = PageView::new();
            let seen = Rc::clone(&splits);
            again.connect_page_split(move || seen.set(seen.get() + 1));
            assert!(again.show_cached(file("wide.png"), PageConfig::default(), split));
            assert_eq!(splits.get(), 2);
        });
    }

    #[test]
    fn a_rotated_page_swaps_its_edges() {
        crate::testing::with_display(|| {
            let page = PageView::new();
            let rotate = DecodeIntent { transform: page_transform::PageTransform::RotateWide { invert: true }, only_if_wide: false };
            page.set_image_from(file("rotate.png"), wide_png(), PageConfig::default(), rotate);
            settle(&page);
            assert_eq!(page.imp().src.get(), Size { w: 200.0, h: 400.0 });
        });
    }

    #[test]
    fn a_webtoon_page_does_not_zoom_on_its_own() {
        crate::testing::with_display(|| {
            let page = PageView::new();
            let window = gtk::Window::new();
            window.set_default_size(800, 600);
            window.set_child(Some(&page));
            window.present();
            page.set_image(png(1000, 2000), PageConfig::default());
            settle(&page);
            settle_all();
            let fit = page.scale();
            page.set_zoomable(false);
            page.imp().pinch_begin();
            page.imp().pinch(3.0);
            assert!((page.scale() - fit).abs() < 1e-9, "locked: the pinch changes nothing");
            page.imp().press(2, 400.0, 300.0, 0);
            settle_all();
            assert!((page.scale() - fit).abs() < 1e-9, "locked: a double tap does not zoom");
            page.set_zoomable(true);
            page.imp().pinch_begin();
            page.imp().pinch(2.0);
            assert!((page.scale() - fit * 2.0).abs() < 1e-9, "unlocked: the pinch scales from the fit");
            window.destroy();
        });
    }
}
