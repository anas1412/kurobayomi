//! The GTK engines (T-44a-010, T-44a-023, T-44a-025, T-44a-032, T-44a-034). Each engine owns its controller, carries
//! out the controller's effects on widgets, and routes pointer, key and scroll input to the controller. Logic stays
//! in the controllers; this module only decides where an effect lands on screen.
//!
//! A page that can be read from disk shows its image in a `PageView`, fitted by the stored scale type and zoom start
//! and following later changes in place (`holder`, `page_opts`). A page that cannot be read here keeps a label that
//! follows the page's status. The page view reports long presses (when `reader_long_tap` is on) as
//! `ViewerEvent::LongTap`; webtoon zoom (T-44a-039) is not wired yet.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::glib::{self, ControlFlow, Propagation};
use gtk::prelude::*;

use super::effect::Effect;
use super::gesture::{self, DragTracker, Gesture, TapRecognizer};
use super::holder::{self, PageHost, Row, Tasks};
use super::keys::{self, InputKey, KeyAction, KeyContext, KeyEvent};
use super::model::{ChapterKey, ChapterSet, ItemKey, PageKind, PageRef};
use super::nav::{self, TapOutcome};
use super::page_opts::page_opts;
use super::pager::controller::{PagerController, PagerOptions};
use super::settings::{self, Family, Reaction, SettingsWatch, ViewerSettings};
use super::webtoon::controller::{WebtoonController, WebtoonOptions};
use super::webtoon::layout::{StripLayout, refresh_range};
use super::{Emitter, ViewerContext, ViewerEngine, ViewerKind};
use crate::reader::model::{ReaderChapter, ReaderPage, ViewerChapters};
use crate::reader::viewer::ViewerEvent;

/// Height of a transition card in the webtoon strip, until its content is measured.
const CARD_HEIGHT: i32 = 200;
/// Length of the scroll animation of taps, keys and swipes.
const SCROLL_MS: i64 = 250;
/// How often the gesture timers are polled (long press and the double-click window).
const POLL_MS: u64 = 20;

/// The chapters and pages a viewer was handed, keyed the way the events name them.
#[derive(Default)]
struct Lookup {
    chapters: HashMap<ChapterKey, Rc<ReaderChapter>>,
    pages: HashMap<PageRef, Rc<ReaderPage>>,
}

impl Lookup {
    fn rebuild(&mut self, chapters: &ViewerChapters) {
        self.chapters.clear();
        self.pages.clear();
        for chapter in [Some(&chapters.curr), chapters.prev.as_ref(), chapters.next.as_ref()].into_iter().flatten() {
            self.chapters.insert(chapter.key(), Rc::clone(chapter));
            for page in chapter.pages().iter().flat_map(|pages| pages.iter()) {
                let r = PageRef { chapter: chapter.key(), index: page.index, kind: PageKind::Normal };
                self.pages.insert(r, Rc::clone(page));
            }
        }
    }

    fn chapter(&self, key: &ChapterKey) -> Option<Rc<ReaderChapter>> {
        self.chapters.get(key).cloned()
    }

    /// A page for an event. An insert shares its parent's bytes (`InsertPage`), so it is made on demand.
    fn page(&self, r: &PageRef) -> Option<Rc<ReaderPage>> {
        match r.kind {
            PageKind::Normal => self.pages.get(r).cloned(),
            PageKind::Insert { .. } => {
                let parent = self.pages.get(&PageRef { kind: PageKind::Normal, ..r.clone() })?;
                Some(ReaderPage::insert_page(parent))
            }
        }
    }
}

fn now_ms() -> u64 {
    (glib::monotonic_time() / 1000).max(0) as u64
}

/// Double-click time, long-press time and drag threshold from the desktop settings (RD §11), with GTK's defaults
/// when no settings object exists. The pager's recogniser acts on release; the webtoon's waits out the window.
fn recognizer(on_release: bool) -> TapRecognizer {
    let (double, long, slop) = match gtk::Settings::default() {
        Some(s) => (
            s.property::<i32>("gtk-double-click-time"),
            // `gtk-long-press-time` is unsigned, the other two are signed.
            i32::try_from(s.property::<u32>("gtk-long-press-time")).unwrap_or(i32::MAX),
            s.property::<i32>("gtk-dnd-drag-threshold"),
        ),
        None => (400, 500, 8),
    };
    let r = TapRecognizer::new(double.max(0) as u64, long.max(0) as u64, f64::from(slop.max(0)));
    if on_release { r.acting_on_release() } else { r }
}

fn input_key(key: &gtk::gdk::Key) -> InputKey {
    match key.name().as_deref() {
        Some("Left") => InputKey::Left,
        Some("Right") => InputKey::Right,
        Some("Up") => InputKey::Up,
        Some("Down") => InputKey::Down,
        Some("Page_Up") => InputKey::PageUp,
        Some("Page_Down") => InputKey::PageDown,
        Some("Menu") => InputKey::Menu,
        Some("XF86AudioRaiseVolume") => InputKey::VolumeUp,
        Some("XF86AudioLowerVolume") => InputKey::VolumeDown,
        _ => InputKey::Other,
    }
}

fn scrolled(horizontal: bool) -> gtk::ScrolledWindow {
    let (h, v) = if horizontal {
        (gtk::PolicyType::External, gtk::PolicyType::Never)
    } else {
        (gtk::PolicyType::Never, gtk::PolicyType::External)
    };
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(h)
        .vscrollbar_policy(v)
        .hexpand(true)
        .vexpand(true)
        .focusable(true)
        .build()
}

/// The row for an item: a label until page content exists, following the page's status or the destination
/// chapter's state (T-44a-006). An insert page follows its parent, whose bytes it shares.
fn item_row(item: &ItemKey, lookup: &Lookup, host: &PageHost) -> Row {
    match item {
        ItemKey::Page(p) => {
            let text = if p.is_insert() {
                format!("Chapter {} page {} (split)", p.chapter.id, p.number())
            } else {
                format!("Chapter {} page {}", p.chapter.id, p.number())
            };
            let parent = PageRef { kind: PageKind::Normal, ..p.clone() };
            // A split insert would show the whole page twice until decode-time splitting exists, so it keeps its label.
            holder::page_row(text, lookup.pages.get(&parent).cloned(), (!p.is_insert()).then_some(host))
        }
        ItemKey::Transition(t) => {
            let text = match (&t.to, t.dir) {
                (Some(to), super::model::Dir::Next) => format!("Next chapter {}", to.id),
                (Some(to), super::model::Dir::Prev) => format!("Previous chapter {}", to.id),
                (None, super::model::Dir::Next) => "There's no next chapter".to_string(),
                (None, super::model::Dir::Prev) => "There's no previous chapter".to_string(),
            };
            holder::transition_row(text, t.to.as_ref().and_then(|to| lookup.chapter(to)))
        }
    }
}

/// What a new page row is handed: the options of the current settings and the long-press report (T-43b-013).
fn page_host(settings: &ViewerSettings, kind: ViewerKind, emit: &Emitter) -> PageHost {
    let emit = emit.clone();
    PageHost {
        opts: page_opts(settings, kind, crate::reader::theme::system_prefers_dark()),
        on_long_tap: Rc::new(move |page, _x, _y| emit.send(ViewerEvent::LongTap(page))),
    }
}

/// Runs a scroll of the adjustment to `target`. Smooth scrolls ease out over `SCROLL_MS`; a newer scroll on the same
/// generation cancels this one. `done` runs when the adjustment arrives.
fn animate(
    root: &gtk::ScrolledWindow,
    adj: gtk::Adjustment,
    generation: Rc<Cell<u64>>,
    target: f64,
    smooth: bool,
    done: impl FnOnce() + 'static,
) {
    let token = generation.get() + 1;
    generation.set(token);
    let start = adj.value();
    // A strip that is not on screen gets no frame clock, so it moves at once rather than never finishing.
    if !smooth || !root.is_mapped() || (start - target).abs() < 0.5 {
        adj.set_value(target);
        done();
        return;
    }
    let started: Cell<Option<i64>> = Cell::new(None);
    let done = RefCell::new(Some(done));
    root.add_tick_callback(move |_, clock| {
        if generation.get() != token {
            return ControlFlow::Break;
        }
        let now = clock.frame_time();
        let t0 = started.get().unwrap_or_else(|| {
            started.set(Some(now));
            now
        });
        let p = ((now - t0) as f64 / (SCROLL_MS * 1000) as f64).clamp(0.0, 1.0);
        adj.set_value(start + (target - start) * (1.0 - (1.0 - p).powi(3)));
        if p < 1.0 {
            return ControlFlow::Continue;
        }
        let finish = done.borrow_mut().take();
        if let Some(finish) = finish {
            finish();
        }
        ControlFlow::Break
    });
}

/// What the pointer, key and wheel handlers need from an engine.
trait Surface: 'static {
    fn root(&self) -> &gtk::ScrolledWindow;
    fn recognizer(&self) -> &RefCell<TapRecognizer>;
    /// Follows the preference store, when the viewer was given one.
    fn watch(&self) -> &RefCell<Option<SettingsWatch>>;
    fn apply_settings(&self, new: ViewerSettings);
    fn alive(&self) -> bool;
    fn tap(&self, gesture: Gesture);
    fn key(&self, ev: KeyEvent) -> bool;
    fn wheel(&self, dy: f64) -> bool;
}

/// Connects the click, motion, key, wheel and gesture-timer controllers of an engine. Handlers hold a weak
/// reference, so a destroyed engine stops receiving input.
fn wire_surface<T: Surface>(this: &Rc<T>) {
    let root = this.root().clone();
    let click = gtk::GestureClick::new();
    let weak = Rc::downgrade(this);
    click.connect_pressed(move |_, _, x, y| {
        let Some(s) = weak.upgrade() else { return };
        let gestures = s.recognizer().borrow_mut().press(now_ms(), x, y);
        gestures.into_iter().for_each(|g| s.tap(g));
    });
    let weak = Rc::downgrade(this);
    click.connect_released(move |_, _, x, y| {
        let Some(s) = weak.upgrade() else { return };
        let gestures = s.recognizer().borrow_mut().release(now_ms(), x, y);
        gestures.into_iter().for_each(|g| s.tap(g));
    });
    root.add_controller(click);

    let motion = gtk::EventControllerMotion::new();
    let weak = Rc::downgrade(this);
    motion.connect_motion(move |_, x, y| {
        if let Some(s) = weak.upgrade() {
            s.recognizer().borrow_mut().motion(x, y);
        }
    });
    root.add_controller(motion);

    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(this);
    keys.connect_key_pressed(move |_, key, _, state| {
        let ev = KeyEvent { key: input_key(&key), ctrl: state.contains(gtk::gdk::ModifierType::CONTROL_MASK), released: false };
        match weak.upgrade() {
            Some(s) if s.key(ev) => Propagation::Stop,
            _ => Propagation::Proceed,
        }
    });
    let weak = Rc::downgrade(this);
    keys.connect_key_released(move |_, key, _, state| {
        let ev = KeyEvent { key: input_key(&key), ctrl: state.contains(gtk::gdk::ModifierType::CONTROL_MASK), released: true };
        if let Some(s) = weak.upgrade() {
            s.key(ev);
        }
    });
    root.add_controller(keys);

    let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let weak = Rc::downgrade(this);
    wheel.connect_scroll(move |_, _, dy| match weak.upgrade() {
        Some(s) if s.wheel(dy) => Propagation::Stop,
        _ => Propagation::Proceed,
    });
    root.add_controller(wheel);

    // Long presses and the double-click window need a clock; the recogniser is polled while the engine lives.
    let weak = Rc::downgrade(this);
    glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        let Some(s) = weak.upgrade() else { return ControlFlow::Break };
        if !s.alive() {
            return ControlFlow::Break;
        }
        let gestures = s.recognizer().borrow_mut().poll(now_ms());
        gestures.into_iter().for_each(|g| s.tap(g));
        // The same clock follows the preference store (T-44a-005): one cheap drain, a read only after a change.
        let changed = s.watch().borrow_mut().as_mut().and_then(SettingsWatch::poll);
        if let Some(new) = changed {
            s.apply_settings(new);
        }
        ControlFlow::Continue
    });
}

/// The tap position as a fraction of the viewer, with the webtoon's height taken from its first layout (B§2.1).
fn normalised(x: f64, y: f64, width: f64, height: f64) -> (f64, f64) {
    ((x / width.max(1.0)).clamp(0.0, 1.0), (y / height.max(1.0)).clamp(0.0, 1.0))
}

pub struct PagerEngine(Rc<PagerInner>);

impl PagerEngine {
    pub fn new(kind: ViewerKind, ctx: ViewerContext) -> Self {
        Self(PagerInner::new(kind, ctx))
    }

    /// Applies a settings change and carries out its reactions (T-44a-005).
    pub fn apply_settings(&self, new: ViewerSettings) {
        self.0.apply_settings(new);
    }
}

impl ViewerEngine for PagerEngine {
    fn kind(&self) -> ViewerKind {
        self.0.kind
    }

    fn widget(&self) -> &gtk::Widget {
        self.0.root.upcast_ref()
    }

    fn set_chapters(&self, chapters: ViewerChapters) {
        self.0.set_chapters(&chapters);
    }

    fn move_to_page(&self, page: &Rc<ReaderPage>) {
        let Some(r) = PageRef::from_reader(page) else { return };
        let fx = self.0.ctl.borrow_mut().move_to_page(&ItemKey::Page(r));
        self.0.run(fx);
    }

    fn handle_key(&self, ev: KeyEvent) -> bool {
        self.0.key(ev)
    }

    fn handle_scroll(&self, dy: f64) -> bool {
        self.0.wheel(dy)
    }

    fn destroy(&self) {
        self.0.destroy();
    }
}

struct PagerInner {
    me: Weak<PagerInner>,
    kind: ViewerKind,
    root: gtk::ScrolledWindow,
    fixed: gtk::Fixed,
    ctl: RefCell<PagerController>,
    lookup: RefCell<Lookup>,
    settings: RefCell<ViewerSettings>,
    emit: Emitter,
    /// The realised rows by index, each with the item it was made for, so a changed item list replaces them.
    slots: RefCell<BTreeMap<usize, (ItemKey, Row)>>,
    recognizer: RefCell<TapRecognizer>,
    watch: RefCell<Option<SettingsWatch>>,
    /// The chapters the core handed over, kept so a chapter's state change can hand them to the controller again.
    chapters: RefCell<Option<ViewerChapters>>,
    chapter_tasks: RefCell<Tasks>,
    anim: Rc<Cell<u64>>,
    /// The index the strip last settled on; a scroll to the same index is not a page change.
    settled: Cell<usize>,
    /// The size the slots were laid out for, so a resize relays them out.
    laid_size: Cell<(i32, i32)>,
}

fn pager_options(s: &ViewerSettings) -> PagerOptions {
    PagerOptions { always_show_transition: s.always_show_transition, use_transitions: s.page_transitions, navigate_pan: s.navigate_pan }
}

impl PagerInner {
    fn new(kind: ViewerKind, ctx: ViewerContext) -> Rc<Self> {
        let root = scrolled(kind != ViewerKind::Vertical);
        let fixed = gtk::Fixed::new();
        root.set_child(Some(&fixed));
        // The strip stays hidden until its first layout (`pager.isVisible = false`).
        root.set_visible(false);
        let settings = ctx.settings;
        let inner = Rc::new_cyclic(|me| PagerInner {
            me: me.clone(),
            kind,
            root,
            fixed,
            ctl: RefCell::new(PagerController::new(kind, pager_options(&settings.clone().before_first_emission()))),
            lookup: RefCell::default(),
            settings: RefCell::new(settings.clone()),
            emit: ctx.emit,
            slots: RefCell::default(),
            recognizer: RefCell::new(recognizer(true)),
            watch: RefCell::new(ctx.prefs.map(|p| SettingsWatch::new(p, settings))),
            chapters: RefCell::default(),
            chapter_tasks: RefCell::default(),
            anim: Rc::default(),
            settled: Cell::new(0),
            laid_size: Cell::new((-1, -1)),
        });
        wire_surface(&inner);
        // The first emission of the store gives page transitions their stored value (E-22).
        let weak = Rc::downgrade(&inner);
        glib::idle_add_local_once(move || {
            if let Some(s) = weak.upgrade() {
                s.ctl.borrow_mut().set_options(pager_options(&s.settings.borrow()));
            }
        });
        inner.wire_drag();
        inner.wire_resize();
        inner
    }

    fn horizontal(&self) -> bool {
        self.kind != ViewerKind::Vertical
    }

    fn extent(&self) -> i32 {
        if self.horizontal() { self.root.width() } else { self.root.height() }.max(0)
    }

    fn adjustment(&self) -> gtk::Adjustment {
        if self.horizontal() { self.root.hadjustment() } else { self.root.vadjustment() }
    }

    /// Takes the chapters and follows their load state: each change hands them to the controller again (B§11.3).
    fn set_chapters(&self, chapters: &ViewerChapters) {
        *self.chapters.borrow_mut() = Some(chapters.clone());
        let weak = self.me.clone();
        let on_change: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                this.apply_chapters();
            }
        });
        *self.chapter_tasks.borrow_mut() = holder::watch_chapters(chapters, on_change);
        self.apply_chapters();
    }

    fn apply_chapters(&self) {
        let Some(chapters) = self.chapters.borrow().clone() else { return };
        self.lookup.borrow_mut().rebuild(&chapters);
        let fx = self.ctl.borrow_mut().set_chapters(ChapterSet::from_viewer(&chapters));
        self.run(fx);
    }

    fn run(&self, fx: Vec<Effect>) {
        for effect in fx {
            match effect {
                Effect::Rebuilt => self.rebuild(),
                Effect::Show => {
                    self.root.set_visible(true);
                    self.rebuild();
                }
                Effect::ScrollTo { index, smooth } => self.scroll_to(index, smooth),
                // Panning a wide page (T-44a-029) and the zoom-start hook (T-44a-03x) are not wired yet.
                Effect::Pan(_) | Effect::ZoomStart { .. } => {}
                Effect::PageSelected(r) => {
                    let page = self.lookup.borrow().page(&r);
                    if let Some(page) = page {
                        self.emit.send(ViewerEvent::PageSelected(page));
                    }
                }
                Effect::HideMenu => self.emit.send(ViewerEvent::HideMenu),
                Effect::ShowMenu => self.emit.send(ViewerEvent::ShowMenu),
                Effect::Preload(key) => {
                    let chapter = self.lookup.borrow().chapter(&key);
                    if let Some(chapter) = chapter {
                        self.emit.send(ViewerEvent::RequestPreload(chapter));
                    }
                }
            }
        }
    }

    /// Realises the slots around the shown index (`current ± 1`) and places them on the strip.
    fn rebuild(&self) {
        let (items, view) = {
            let ctl = self.ctl.borrow();
            (ctl.items().to_vec(), ctl.view_index())
        };
        let (w, h) = (self.root.width().max(0), self.root.height().max(0));
        self.laid_size.set((w, h));
        let count = items.len();
        let wanted: Range<usize> = if count == 0 { 0..0 } else { view.saturating_sub(1)..(view + 2).min(count) };
        let mut slots = self.slots.borrow_mut();
        let stale: Vec<usize> = slots.keys().filter(|i| !wanted.contains(i)).copied().collect();
        for i in stale {
            if let Some((_, row)) = slots.remove(&i) {
                self.fixed.remove(&row.widget);
            }
        }
        for i in wanted {
            if slots.get(&i).is_some_and(|(key, _)| *key != items[i]) {
                if let Some((_, old)) = slots.remove(&i) {
                    self.fixed.remove(&old.widget);
                }
            }
            let widget = slots
                .entry(i)
                .or_insert_with(|| {
                    let row = item_row(&items[i], &self.lookup.borrow(), &page_host(&self.settings.borrow(), self.kind, &self.emit));
                    self.fixed.put(&row.widget, 0.0, 0.0);
                    (items[i].clone(), row)
                })
                .1
                .widget
                .clone();
            let (x, y) = if self.horizontal() { (i as i32 * w, 0) } else { (0, i as i32 * h) };
            self.fixed.move_(&widget, f64::from(x), f64::from(y));
            widget.set_size_request(w, h);
        }
        let total = count as i32;
        if self.horizontal() {
            self.fixed.set_size_request(total * w, h);
        } else {
            self.fixed.set_size_request(w, total * h);
        }
    }

    fn scroll_to(&self, index: usize, smooth: bool) {
        self.rebuild();
        let target = index as f64 * f64::from(self.extent());
        self.glide(target, smooth, Some(index));
    }

    /// Moves the strip to `target`. While a smooth move runs the strip is not idle, so a chapter set waits for it
    /// (E-75). When the move ends, the strip settles on `settle_to` (if the page changed) and becomes idle again.
    fn glide(&self, target: f64, smooth: bool, settle_to: Option<usize>) {
        if smooth {
            self.set_idle(false);
        }
        let weak = self.me.clone();
        animate(&self.root, self.adjustment(), Rc::clone(&self.anim), target, smooth, move || {
            if let Some(s) = weak.upgrade() {
                if let Some(index) = settle_to {
                    s.settle(index);
                }
                s.set_idle(true);
            }
        });
    }

    fn set_idle(&self, idle: bool) {
        let fx = self.ctl.borrow_mut().set_idle(idle);
        self.run(fx);
    }

    /// The strip arrived on `index` (ViewPager `onPageSelected`). A scroll that ends where it started is not a
    /// page change.
    fn settle(&self, index: usize) {
        if self.settled.replace(index) == index {
            self.rebuild();
            return;
        }
        self.rebuild();
        let fx = self.ctl.borrow_mut().on_settled(index);
        self.run(fx);
    }

    /// Swipe commit (B§5, TP §5): past half the extent, or on a release velocity above the threshold, to the
    /// neighbour the drag points to. Otherwise it snaps back.
    fn finish_drag(&self, offset: f64, velocity: f64) {
        let extent = f64::from(self.extent());
        let (view, count) = {
            let ctl = self.ctl.borrow();
            (ctl.view_index(), ctl.items().len())
        };
        let step = gesture::swipe_step(offset, velocity, extent);
        let target = view as i64 + step;
        if step != 0 && target >= 0 && (target as usize) < count {
            let index = target as usize;
            self.glide(index as f64 * extent, true, Some(index));
        } else {
            self.glide(view as f64 * extent, true, None);
        }
    }

    fn wire_drag(&self) {
        let drag = gtk::GestureDrag::new();
        let start = Rc::new(Cell::new(0.0));
        let track = Rc::new(RefCell::new(DragTracker::default()));
        let weak = self.me.clone();
        let (s, t) = (Rc::clone(&start), Rc::clone(&track));
        drag.connect_drag_begin(move |_, _, _| {
            if let Some(this) = weak.upgrade() {
                t.borrow_mut().begin();
                s.set(this.adjustment().value());
                this.rebuild();
                this.set_idle(false);
            }
        });
        let weak = self.me.clone();
        let (s, t) = (Rc::clone(&start), Rc::clone(&track));
        drag.connect_drag_update(move |_, dx, dy| {
            if let Some(this) = weak.upgrade() {
                let offset = if this.horizontal() { dx } else { dy };
                t.borrow_mut().push(now_ms(), offset);
                this.adjustment().set_value(s.get() - offset);
            }
        });
        let weak = self.me.clone();
        drag.connect_drag_end(move |_, dx, dy| {
            if let Some(this) = weak.upgrade() {
                let offset = if this.horizontal() { dx } else { dy };
                // The release is a sample too, so a drag that paused before letting go has no velocity.
                let velocity = {
                    let mut track = track.borrow_mut();
                    track.push(now_ms(), offset);
                    track.velocity()
                };
                this.finish_drag(offset, velocity);
            }
        });
        self.root.add_controller(drag);
    }

    /// A resize moves the slots and the strip to the new extent. Checked once per frame, which is cheap.
    fn wire_resize(&self) {
        let weak = self.me.clone();
        self.root.add_tick_callback(move |root, _| {
            let Some(this) = weak.upgrade() else { return ControlFlow::Break };
            if this.laid_size.get() != (root.width(), root.height()) && root.is_visible() {
                this.rebuild();
                let view = this.ctl.borrow().view_index() as f64;
                this.adjustment().set_value(view * f64::from(this.extent()));
            }
            ControlFlow::Continue
        });
    }

    fn command(&self, cmd: super::pager::nav::PageCmd) {
        let fx = self.ctl.borrow_mut().command(cmd, false);
        self.run(fx);
    }

    fn set_splits_removed(&self) {
        let fx = self.ctl.borrow_mut().remove_splits();
        self.run(fx);
    }

    fn apply_settings(&self, new: ViewerSettings) {
        let old = std::mem::replace(&mut *self.settings.borrow_mut(), new.clone());
        self.ctl.borrow_mut().set_options(pager_options(&new));
        let opts = page_opts(&new, self.kind, crate::reader::theme::system_prefers_dark());
        self.slots.borrow().values().for_each(|(_, row)| row.apply(&opts));
        for reaction in settings::diff(&old, &new, Family::Pager) {
            match reaction {
                Reaction::RefreshImages => {
                    let fx = self.ctl.borrow().refresh();
                    self.run(fx);
                }
                Reaction::RemoveSplits => self.set_splits_removed(),
                // The taps read the table on each release. The pager's theme change refreshes its images, and it has
                // no webtoon zoom, so the other reactions do not apply to it.
                Reaction::Navigation | Reaction::Restyle | Reaction::ZoomOutDisabled | Reaction::DoubleTapZoom => {}
            }
        }
    }

    fn destroy(&self) {
        self.emit.close();
        self.anim.set(self.anim.get() + 1);
        *self.chapter_tasks.borrow_mut() = Tasks::default();
        let mut child = self.fixed.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            self.fixed.remove(&widget);
        }
        self.slots.borrow_mut().clear();
    }

    fn key(&self, ev: KeyEvent) -> bool {
        let ctx = key_context(&self.settings.borrow());
        let out = keys::pager_key(ev, ctx);
        match out.action {
            Some(KeyAction::Page(cmd)) => self.command(cmd),
            Some(KeyAction::ToggleMenu) => self.emit.send(ViewerEvent::ToggleMenu),
            Some(KeyAction::Scroll { .. }) | None => {}
        }
        out.claimed
    }

    /// A pointer scroll: down is the next page, and up or no movement is the previous one (E-24).
    fn wheel(&self, dy: f64) -> bool {
        self.command(if dy > 0.0 { super::pager::nav::PageCmd::Next } else { super::pager::nav::PageCmd::Prev });
        true
    }
}

fn key_context(s: &ViewerSettings) -> KeyContext {
    KeyContext { volume_enabled: s.volume_keys, volume_inverted: s.volume_keys_inverted, menu_visible: false }
}

impl Surface for PagerInner {
    fn root(&self) -> &gtk::ScrolledWindow {
        &self.root
    }

    fn recognizer(&self) -> &RefCell<TapRecognizer> {
        &self.recognizer
    }

    fn watch(&self) -> &RefCell<Option<SettingsWatch>> {
        &self.watch
    }

    fn apply_settings(&self, new: ViewerSettings) {
        PagerInner::apply_settings(self, new);
    }

    fn alive(&self) -> bool {
        self.emit.is_open()
    }

    fn tap(&self, gesture: Gesture) {
        // Long presses reach the page view, and a pager double tap is swallowed (E-20).
        let Gesture::Tap { x, y } = gesture else { return };
        let (nx, ny) = normalised(x, y, f64::from(self.root.width()), f64::from(self.root.height()));
        let outcome = {
            let s = self.settings.borrow();
            let action = nav::hit_test(nav::layout(s.nav_mode_pager, self.kind), s.tapping_pager, nx, ny);
            nav::pager_tap(action)
        };
        match outcome {
            TapOutcome::ToggleMenu => self.emit.send(ViewerEvent::ToggleMenu),
            TapOutcome::Page(cmd) => self.command(cmd),
            TapOutcome::ScrollDown | TapOutcome::ScrollUp => {}
        }
    }

    fn key(&self, ev: KeyEvent) -> bool {
        PagerInner::key(self, ev)
    }

    fn wheel(&self, dy: f64) -> bool {
        PagerInner::wheel(self, dy)
    }
}

pub struct WebtoonEngine(Rc<WebtoonInner>);

impl WebtoonEngine {
    pub fn new(gapped: bool, ctx: ViewerContext) -> Self {
        Self(WebtoonInner::new(gapped, ctx))
    }

    /// Applies a settings change and carries out its reactions (T-44a-005).
    pub fn apply_settings(&self, new: ViewerSettings) {
        self.0.apply_settings(new);
    }
}

impl ViewerEngine for WebtoonEngine {
    fn kind(&self) -> ViewerKind {
        self.0.kind
    }

    fn widget(&self) -> &gtk::Widget {
        self.0.root.upcast_ref()
    }

    fn set_chapters(&self, chapters: ViewerChapters) {
        self.0.set_chapters(&chapters);
    }

    fn move_to_page(&self, page: &Rc<ReaderPage>) {
        let Some(r) = PageRef::from_reader(page) else { return };
        let fx = self.0.ctl.borrow_mut().move_to_page(&ItemKey::Page(r));
        self.0.run(fx);
    }

    fn handle_key(&self, ev: KeyEvent) -> bool {
        self.0.key(ev)
    }

    /// The scrolled window handles the wheel itself (B§4.4).
    fn handle_scroll(&self, _dy: f64) -> bool {
        false
    }

    fn destroy(&self) {
        self.0.destroy();
    }
}

struct WebtoonInner {
    me: Weak<WebtoonInner>,
    kind: ViewerKind,
    root: gtk::ScrolledWindow,
    fixed: gtk::Fixed,
    ctl: RefCell<WebtoonController>,
    lookup: RefCell<Lookup>,
    settings: RefCell<ViewerSettings>,
    emit: Emitter,
    /// Realised rows, keyed by item so a rebuild reuses the rows that survive it (the keyed diff of T-44a-031).
    realised: RefCell<HashMap<ItemKey, Row>>,
    layout: RefCell<StripLayout>,
    watch: RefCell<Option<SettingsWatch>>,
    /// The chapters the core handed over, kept so a chapter's state change can hand them to the controller again.
    chapters: RefCell<Option<ViewerChapters>>,
    chapter_tasks: RefCell<Tasks>,
    /// The size the strip was last laid out for. A strip that is shown before it has a size is laid out again once
    /// it has one (T-44a-032).
    laid_size: Cell<(i32, i32)>,
    /// An item to scroll to once the scroll range has grown to the strip. Set by a scroll that came before the
    /// strip was allocated; applied on the frame it has been.
    pending_scroll: Cell<Option<usize>>,
    last_scroll: Cell<i64>,
    /// Set while `sync` measures and reconciles, so the value changes it causes do not re-enter it.
    busy: Cell<bool>,
    /// The viewport height at the first layout. Taps are normalised by it (B§2.1).
    original_height: Cell<i32>,
    recognizer: RefCell<TapRecognizer>,
    anim: Rc<Cell<u64>>,
    /// False until the store's first emission, so page transitions start off (E-22).
    transitions_live: Cell<bool>,
}

fn webtoon_options(s: &ViewerSettings) -> WebtoonOptions {
    WebtoonOptions { always_show_transition: s.always_show_transition, use_transitions: s.page_transitions, hide_threshold: s.hide_threshold }
}

impl WebtoonInner {
    fn new(gapped: bool, ctx: ViewerContext) -> Rc<Self> {
        let root = scrolled(false);
        let fixed = gtk::Fixed::new();
        root.set_child(Some(&fixed));
        root.set_visible(false);
        let kind = if gapped { ViewerKind::WebtoonGapped } else { ViewerKind::Webtoon };
        let settings = ctx.settings;
        let inner = Rc::new_cyclic(|me| WebtoonInner {
            me: me.clone(),
            kind,
            root,
            fixed,
            ctl: RefCell::new(WebtoonController::new(webtoon_options(&settings.clone().before_first_emission()))),
            lookup: RefCell::default(),
            settings: RefCell::new(settings.clone()),
            emit: ctx.emit,
            realised: RefCell::default(),
            layout: RefCell::default(),
            watch: RefCell::new(ctx.prefs.map(|p| SettingsWatch::new(p, settings))),
            chapters: RefCell::default(),
            chapter_tasks: RefCell::default(),
            laid_size: Cell::new((-1, -1)),
            pending_scroll: Cell::new(None),
            last_scroll: Cell::new(0),
            busy: Cell::new(false),
            original_height: Cell::new(0),
            recognizer: RefCell::new(recognizer(false)),
            anim: Rc::default(),
            transitions_live: Cell::new(false),
        });
        wire_surface(&inner);
        let weak = Rc::downgrade(&inner);
        glib::idle_add_local_once(move || {
            if let Some(s) = weak.upgrade() {
                s.transitions_live.set(true);
                s.ctl.borrow_mut().set_options(webtoon_options(&s.settings.borrow()));
            }
        });
        let weak = inner.me.clone();
        let adj = inner.root.vadjustment();
        adj.connect_value_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.sync();
            }
        });
        inner.wire_resize();
        inner
    }

    /// The strip is laid out once it has a size, and again when the size changes. Checked once per frame, like the
    /// pager. The scroll range grows a frame after the layout sets the strip's size, so a pending scroll waits for it.
    fn wire_resize(&self) {
        let weak = self.me.clone();
        self.root.add_tick_callback(move |root, _| {
            let Some(this) = weak.upgrade() else { return ControlFlow::Break };
            if !root.is_visible() || root.height() <= 0 {
                return ControlFlow::Continue;
            }
            let size = (root.width(), root.height());
            let resized = this.laid_size.replace(size) != size;
            if this.pending_scroll.get().is_some() {
                // Size the strip first, but report nothing before the scroll lands: the controller would take the
                // top of the strip for the current item.
                if resized {
                    this.lay_out();
                }
                if this.apply_pending_scroll() {
                    this.sync();
                }
            } else if resized {
                this.sync();
            }
            ControlFlow::Continue
        });
    }

    /// Scrolls to the pending item when the scroll range holds the strip. `true` when it did.
    fn apply_pending_scroll(&self) -> bool {
        let Some(index) = self.pending_scroll.get() else { return false };
        let (_, layout) = self.current_layout();
        let adj = self.root.vadjustment();
        if !layout.fits_range(adj.upper()) {
            return false;
        }
        self.pending_scroll.set(None);
        let target = (layout.scroll_to(index) as f64).min(adj.upper() - adj.page_size()).max(adj.lower());
        // A jump is not a scroll the user made (B§7.5: the view reports it with `dy == 0`).
        self.last_scroll.set(target.round() as i64);
        adj.set_value(target);
        true
    }

    fn viewport(&self) -> i32 {
        let h = self.root.height().max(0);
        if self.original_height.get() == 0 && h > 0 {
            self.original_height.set(h);
        }
        h
    }

    /// The strip geometry for the current items: pages fill the viewport until decoded, cards have a fixed height.
    fn current_layout(&self) -> (Vec<ItemKey>, StripLayout) {
        let items = self.ctl.borrow().items().to_vec();
        let vh = self.viewport();
        let heights = items.iter().map(|i| if i.page().is_some() { vh } else { CARD_HEIGHT }).collect();
        (items, StripLayout::new(heights))
    }

    /// Takes the chapters and follows their load state: each change hands them to the controller again (B§11.3).
    fn set_chapters(&self, chapters: &ViewerChapters) {
        *self.chapters.borrow_mut() = Some(chapters.clone());
        let weak = self.me.clone();
        let on_change: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                this.apply_chapters();
            }
        });
        *self.chapter_tasks.borrow_mut() = holder::watch_chapters(chapters, on_change);
        self.apply_chapters();
    }

    fn apply_chapters(&self) {
        let Some(chapters) = self.chapters.borrow().clone() else { return };
        self.lookup.borrow_mut().rebuild(&chapters);
        let fx = self.ctl.borrow_mut().set_chapters(ChapterSet::from_viewer(&chapters));
        self.run(fx);
    }

    fn run(&self, fx: Vec<Effect>) {
        for effect in fx {
            match effect {
                Effect::Rebuilt => self.sync(),
                Effect::Show => {
                    self.root.set_visible(true);
                    self.sync();
                }
                Effect::ScrollTo { index, .. } => {
                    // The strip may not be allocated yet; the frame tick finishes the scroll then.
                    self.pending_scroll.set(Some(index));
                    if self.apply_pending_scroll() {
                        self.sync();
                    } else {
                        self.lay_out();
                    }
                }
                Effect::Pan(_) | Effect::ZoomStart { .. } => {}
                Effect::PageSelected(r) => {
                    let page = self.lookup.borrow().page(&r);
                    if let Some(page) = page {
                        self.emit.send(ViewerEvent::PageSelected(page));
                    }
                }
                Effect::HideMenu => self.emit.send(ViewerEvent::HideMenu),
                Effect::ShowMenu => self.emit.send(ViewerEvent::ShowMenu),
                Effect::Preload(key) => {
                    let chapter = self.lookup.borrow().chapter(&key);
                    if let Some(chapter) = chapter {
                        self.emit.send(ViewerEvent::RequestPreload(chapter));
                    }
                }
            }
        }
    }

    /// Realises the rows around the viewport, reports the scroll to the controller and carries out what it asks.
    /// Re-entrant calls (a size change moves the scroll value) are dropped while this measures.
    fn sync(&self) {
        // Before the first allocation every placeholder is 0 px tall, so nothing would realise and no item would be
        // current. The frame tick lays the strip out once it has a size (T-44a-032).
        if self.root.height() <= 0 || self.busy.replace(true) {
            return;
        }
        let fx = self.measure();
        self.busy.set(false);
        self.run(fx);
    }

    fn measure(&self) -> Vec<Effect> {
        let scroll = self.root.vadjustment().value().round() as i64;
        let dy = scroll - self.last_scroll.replace(scroll);
        let (last, first) = self.lay_out();
        self.ctl.borrow_mut().on_scroll(dy as i32, true, last, first)
    }

    /// Realises the rows for the scroll position and sizes the strip. Returns the bottom-most item whose end is
    /// visible and the first visible item, which the controller needs to report the current item.
    fn lay_out(&self) -> (Option<usize>, Option<usize>) {
        let vh = self.viewport();
        if vh <= 0 {
            return (None, None);
        }
        let scroll = self.root.vadjustment().value().round() as i64;
        let (items, layout) = self.current_layout();
        let range = layout.realised(scroll, vh);
        let last = layout.last_end_visible(scroll, vh, range.clone());
        let first = layout.first_visible(scroll);
        self.reconcile(&items, &layout, range);
        (last, first)
    }

    fn reconcile(&self, items: &[ItemKey], layout: &StripLayout, range: Range<usize>) {
        let width = self.root.width().max(0);
        let mut realised = self.realised.borrow_mut();
        let wanted: HashSet<&ItemKey> = range.clone().map(|i| &items[i]).collect();
        let stale: Vec<ItemKey> = realised.keys().filter(|k| !wanted.contains(k)).cloned().collect();
        for key in stale {
            if let Some(row) = realised.remove(&key) {
                self.fixed.remove(&row.widget);
            }
        }
        for i in range {
            let key = &items[i];
            let widget = realised
                .entry(key.clone())
                .or_insert_with(|| {
                    let row = item_row(key, &self.lookup.borrow(), &page_host(&self.settings.borrow(), self.kind, &self.emit));
                    self.fixed.put(&row.widget, 0.0, 0.0);
                    row
                })
                .widget
                .clone();
            widget.set_size_request(width, layout.height(i));
            self.fixed.move_(&widget, 0.0, layout.offset(i) as f64);
        }
        drop(realised);
        self.fixed.set_size_request(width, layout.total_height() as i32);
        *self.layout.borrow_mut() = layout.clone();
    }

    /// Scrolls by `delta` pixels, animated when page transitions are on (B§7.4).
    fn scroll_by(&self, delta: f64) {
        let adj = self.root.vadjustment();
        let target = adj.value() + delta;
        let smooth = self.transitions_live.get() && self.settings.borrow().page_transitions;
        animate(&self.root, adj, Rc::clone(&self.anim), target, smooth, || {});
    }

    /// One scroll step is three quarters of the viewport (B§7.4).
    fn step(&self) -> f64 {
        f64::from(StripLayout::scroll_distance(self.root.height().max(0)))
    }

    /// `refreshAdapter` (B§7.7, E-30): drops the rows in `[centre-3, centre+3]` so they are rebuilt.
    fn refresh_images(&self) {
        let scroll = self.root.vadjustment().value().round() as i64;
        let vh = self.viewport();
        let (items, layout) = self.current_layout();
        let centre = layout.last_end_visible(scroll, vh, layout.realised(scroll, vh));
        if let Some(range) = refresh_range(centre, items.len()) {
            let mut realised = self.realised.borrow_mut();
            let dropped: Vec<Row> = range.filter_map(|i| realised.remove(&items[i])).collect();
            drop(realised);
            dropped.iter().for_each(|row| self.fixed.remove(&row.widget));
        }
        self.sync();
    }

    fn apply_settings(&self, new: ViewerSettings) {
        let old = std::mem::replace(&mut *self.settings.borrow_mut(), new.clone());
        self.ctl.borrow_mut().set_options(webtoon_options(&new));
        let opts = page_opts(&new, self.kind, crate::reader::theme::system_prefers_dark());
        self.realised.borrow().values().for_each(|row| row.apply(&opts));
        for reaction in settings::diff(&old, &new, Family::Webtoon) {
            match reaction {
                Reaction::RefreshImages => self.refresh_images(),
                // The webtoon does not restart for a theme change as upstream does; its background changes in place.
                Reaction::Restyle => crate::reader::theme::apply_background(&self.root, new.reader_theme),
                // The taps read the table on each release. Zoom-out and double-tap zoom are read from the live
                // settings by the zoom frame (T-44a-039, T-44a-042, T-44a-043), which does not exist yet, so
                // there is nothing to refresh until it does.
                Reaction::Navigation | Reaction::RemoveSplits | Reaction::ZoomOutDisabled | Reaction::DoubleTapZoom => {}
            }
        }
    }

    fn destroy(&self) {
        self.emit.close();
        self.anim.set(self.anim.get() + 1);
        *self.chapter_tasks.borrow_mut() = Tasks::default();
        let mut child = self.fixed.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            self.fixed.remove(&widget);
        }
        self.realised.borrow_mut().clear();
    }

    fn key(&self, ev: KeyEvent) -> bool {
        let ctx = key_context(&self.settings.borrow());
        let out = keys::webtoon_key(ev, ctx);
        match out.action {
            Some(KeyAction::Scroll { down }) => {
                let step = self.step();
                self.scroll_by(if down { step } else { -step });
            }
            Some(KeyAction::ToggleMenu) => self.emit.send(ViewerEvent::ToggleMenu),
            Some(KeyAction::Page(_)) | None => {}
        }
        out.claimed
    }
}

impl Surface for WebtoonInner {
    fn root(&self) -> &gtk::ScrolledWindow {
        &self.root
    }

    fn recognizer(&self) -> &RefCell<TapRecognizer> {
        &self.recognizer
    }

    fn watch(&self) -> &RefCell<Option<SettingsWatch>> {
        &self.watch
    }

    fn apply_settings(&self, new: ViewerSettings) {
        WebtoonInner::apply_settings(self, new);
    }

    fn alive(&self) -> bool {
        self.emit.is_open()
    }

    fn tap(&self, gesture: Gesture) {
        // Double tap zoom (T-44a-039) is not wired in this slice. Long presses reach the page view.
        let Gesture::Tap { x, y } = gesture else { return };
        let original = f64::from(self.original_height.get());
        let (nx, ny) = normalised(x, y, f64::from(self.root.width()), original);
        let outcome = {
            let s = self.settings.borrow();
            let action = nav::hit_test(nav::layout(s.nav_mode_webtoon, self.kind), s.tapping_webtoon, nx, ny);
            nav::webtoon_tap(action)
        };
        match outcome {
            TapOutcome::ToggleMenu => self.emit.send(ViewerEvent::ToggleMenu),
            TapOutcome::ScrollDown => self.scroll_by(self.step()),
            TapOutcome::ScrollUp => self.scroll_by(-self.step()),
            TapOutcome::Page(_) => {}
        }
    }

    fn key(&self, ev: KeyEvent) -> bool {
        WebtoonInner::key(self, ev)
    }

    fn wheel(&self, _dy: f64) -> bool {
        false
    }
}
