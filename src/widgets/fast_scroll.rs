//! Fast scroller: pure proportional mapping (T-49a-047), the list thumb and its fade (T-49a-031),
//! and the list and grid constructors (T-49a-029).
//!
//! GTK scrolls by pixels, so the thumb maps by proportion of the scroll range and not by item
//! index (`p1-rust-design.md` 9.1). The index helper keeps the list-of-items mapping for grids.
//! The thumb is hidden when the content fits (E-49-17), shows on scroll for 2 s, then fades in 250 ms.
//!
//! The thumb is pointer-only (T-49a-055): it duplicates the scrolled window, which already scrolls
//! with the arrow, Page Up and Page Down, Home and End keys, so it takes no focus.
//!
//! `list` and `grid` build the content (a `ListBox` or a `FlowBox`) and wrap it; the caller fills the
//! content and picks `Mode::Passive` (`ScrollbarLazyColumn`) or `Mode::Fast` (`FastScrollLazyColumn`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

use crate::theme::tokens::{Rgba, SCROLLBAR_FADE_DELAY_MS, THUMB_LENGTH, THUMB_THICKNESS};
use super::layout::column_count;

/// Key prefix of sticky headers in the Compose list. Such rows are skipped for the thumb (E-49-14).
pub const STICKY_PREFIX: &str = "sticky:";
/// `Dp.Hairline` in the source, the default edge padding.
pub const HAIRLINE: f64 = 0.5;
/// Gap between the thumb and the right edge (the source's `8.dp` end inset).
pub const THUMB_END_INSET: f64 = 8.0;

/// Thumb top for a scroll position. `range` is the full scroll range, `view` the viewport height.
/// Proportion is clamped, and a list that fits (`range <= view`) keeps the thumb at the top.
pub fn thumb_top(scroll: f64, range: f64, view: f64, track: f64, top_pad: f64) -> f64 {
    let span = range - view;
    let proportion = if span > 0.0 { (scroll / span).clamp(0.0, 1.0) } else { 0.0 };
    top_pad + track * proportion
}

/// Scroll offset for a thumb position (drag). The thumb position is clamped to the track first.
pub fn scroll_for_thumb(thumb_y: f64, track: f64, top_pad: f64, range: f64, view: f64) -> f64 {
    let ratio = if track > 0.0 { ((thumb_y - top_pad) / track).clamp(0.0, 1.0) } else { 0.0 };
    ratio * (range - view).max(0.0)
}

/// Length of the track the thumb moves along: the viewport minus the padding and the thumb.
pub fn track_length(view: f64, top_pad: f64, bottom_pad: f64) -> f64 {
    (view - top_pad - bottom_pad - f64::from(THUMB_LENGTH)).max(0.0)
}

/// The item a drag ratio points at, proportional over `total` items (E-49-16, TEST-49-13).
pub fn item_index_for_ratio(ratio: f64, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    ((ratio.clamp(0.0, 1.0) * total as f64).round() as usize).min(total - 1)
}

/// The first visible row that is not a sticky header. Falls back to the first row when every
/// visible row is sticky, instead of panicking (E-49-14). `None` only when nothing is visible.
pub fn first_non_sticky(keys: &[Option<&str>]) -> Option<usize> {
    if keys.is_empty() {
        return None;
    }
    let found = keys
        .iter()
        .position(|k| !k.is_some_and(|k| k.starts_with(STICKY_PREFIX)));
    Some(found.unwrap_or(0))
}

/// Thumb opacity `elapsed_ms` after the last scroll: full for `delay_ms`, then a linear fade over
/// `fade_ms`. A zero fade drops straight to 0.
pub fn thumb_opacity(elapsed_ms: u64, delay_ms: u64, fade_ms: u64) -> f64 {
    if elapsed_ms <= delay_ms {
        return 1.0;
    }
    if fade_ms == 0 {
        return 0.0;
    }
    (1.0 - (elapsed_ms - delay_ms) as f64 / fade_ms as f64).clamp(0.0, 1.0)
}

/// The thumb shows only when the content overflows its viewport (E-49-17).
pub fn overflows(range: f64, view: f64) -> bool {
    range - view > 0.0
}

/// Grid column layout: the minimum cell width and the spacing between cells. The fields are private
/// so a layout always has a positive cell width: `new` is the build-time check for a bounded grid
/// (an unbounded cell cannot be built, so no list fallback and no runtime panic).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLayout {
    min_cell: i32,
    spacing: i32,
}

/// Why a `GridLayout` was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridError {
    /// The cell width is zero or negative, so the column count is unbounded.
    UnboundedCell,
    /// The spacing is negative.
    NegativeSpacing,
}

impl std::fmt::Display for GridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridError::UnboundedCell => write!(f, "a fast scroll grid needs a positive cell width"),
            GridError::NegativeSpacing => write!(f, "a fast scroll grid needs a spacing of zero or more"),
        }
    }
}

impl std::error::Error for GridError {}

impl GridLayout {
    pub fn new(min_cell: i32, spacing: i32) -> Result<Self, GridError> {
        if min_cell <= 0 {
            return Err(GridError::UnboundedCell);
        }
        if spacing < 0 {
            return Err(GridError::NegativeSpacing);
        }
        Ok(GridLayout { min_cell, spacing })
    }

    pub fn min_cell(self) -> i32 {
        self.min_cell
    }

    pub fn spacing(self) -> i32 {
        self.spacing
    }

    /// Columns that fit `width` (at least one).
    pub fn columns_for(self, width: i32) -> usize {
        column_count(width, self.min_cell, self.spacing)
    }
}

/// Edge paddings around the thumb track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paddings {
    pub top: f64,
    pub bottom: f64,
    pub end: f64,
}

impl Default for Paddings {
    fn default() -> Self {
        Paddings { top: HAIRLINE, bottom: HAIRLINE, end: HAIRLINE }
    }
}

/// Which scroller to build (`ScrollbarLazyColumn` or `FastScrollLazyColumn`, T-49a-029).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// The GTK overlay scrollbar only.
    Passive,
    /// The overlay scrollbar plus the fast-scroll thumb.
    Fast,
}

/// The thumb colour: the theme accent, or one colour for this scroller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThumbColor {
    #[default]
    Accent,
    Fixed(Rgba),
}

/// The fast scroller options: mode, thumb predicate, thumb colour and edge paddings.
pub struct Options {
    pub mode: Mode,
    /// `thumb_allowed`: false fades the thumb out at once, with no delay (`p1-topic` section 5).
    pub thumb_allowed: Rc<dyn Fn() -> bool>,
    pub thumb_color: ThumbColor,
    pub paddings: Paddings,
}

impl Options {
    /// The mode with the default predicate, accent colour and paddings.
    pub fn new(mode: Mode) -> Self {
        Options {
            mode,
            thumb_allowed: Rc::new(|| true),
            thumb_color: ThumbColor::Accent,
            paddings: Paddings::default(),
        }
    }
}

/// A scroller and its content, for the caller to fill.
pub struct FastScroll<C> {
    pub root: gtk::Overlay,
    pub content: C,
}

/// A list scroller around a `ListBox`.
pub fn list(options: Options) -> FastScroll<gtk::ListBox> {
    let content = gtk::ListBox::new();
    content.set_selection_mode(gtk::SelectionMode::None);
    let (root, _) = assemble(&content, options);
    FastScroll { root, content }
}

/// A grid scroller around a `FlowBox`. The column count is recomputed from the viewport width
/// whenever it changes (E-49-15).
pub fn grid(layout: GridLayout, options: Options) -> FastScroll<gtk::FlowBox> {
    let content = gtk::FlowBox::new();
    content.set_selection_mode(gtk::SelectionMode::None);
    content.set_homogeneous(true);
    content.set_min_children_per_line(1);
    content.set_column_spacing(layout.spacing() as u32);
    content.set_row_spacing(layout.spacing() as u32);
    content.set_valign(gtk::Align::Start);
    let (root, scrolled) = assemble(&content, options);
    scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);

    let columns = Rc::new(Cell::new(0usize));
    let recompute = {
        let flow = content.downgrade();
        let adj = scrolled.hadjustment();
        move || {
            let Some(flow) = flow.upgrade() else { return };
            let next = layout.columns_for(adj.page_size() as i32);
            if columns.replace(next) != next {
                flow.set_max_children_per_line(next as u32);
            }
        }
    };
    let recompute = Rc::new(recompute);
    {
        let recompute = recompute.clone();
        scrolled.hadjustment().connect_changed(move |_| recompute());
    }
    recompute();
    FastScroll { root, content }
}

/// Wraps `content` in a scroller. `Passive` gives the overlay scrollbar only; `Fast` adds the thumb.
pub fn attach(content: &impl IsA<gtk::Widget>, options: Options) -> gtk::Overlay {
    assemble(content, options).0
}

fn assemble(content: &impl IsA<gtk::Widget>, options: Options) -> (gtk::Overlay, gtk::ScrolledWindow) {
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(content));
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&scrolled));
    if options.mode == Mode::Fast {
        add_thumb(&overlay, &scrolled, options);
    }
    (overlay, scrolled)
}

/// The thumb: positions from the vertical adjustment, shows on scroll, fades after the hold, and
/// follows a drag (R-49-05: a drag ignores the adjustment's own changes while it runs).
fn add_thumb(overlay: &gtk::Overlay, scrolled: &gtk::ScrolledWindow, options: Options) {
    let thumb = gtk::Box::new(gtk::Orientation::Vertical, 0);
    thumb.add_css_class(crate::theme::style::class::FAST_THUMB);
    thumb.set_size_request(THUMB_THICKNESS as i32, THUMB_LENGTH as i32);
    thumb.set_halign(gtk::Align::End);
    thumb.set_valign(gtk::Align::Start);
    thumb.set_margin_end(THUMB_END_INSET as i32 + options.paddings.end as i32);
    overlay.add_overlay(&thumb);
    if let ThumbColor::Fixed(color) = options.thumb_color {
        apply_thumb_color(&thumb, color);
    }

    let adj = scrolled.vadjustment();
    let state = Rc::new(ThumbState::new(options.paddings));
    let allowed = options.thumb_allowed.clone();

    let place = {
        let thumb = thumb.clone();
        let adj = adj.clone();
        let pads = options.paddings;
        move || {
            let (view, range) = (adj.page_size(), adj.upper());
            let track = track_length(view, pads.top, pads.bottom);
            let top = thumb_top(adj.value(), range, view, track, pads.top);
            thumb.set_margin_top(top.round() as i32);
            thumb.set_visible(overflows(range, view));
        }
    };
    let place = Rc::new(place);
    {
        let state = state.clone();
        let place = place.clone();
        let thumb = thumb.clone();
        adj.connect_value_changed(move |_| {
            if state.dragging.get() {
                return;
            }
            place();
            show_then_fade(&thumb, &state, allowed());
        });
    }
    {
        let place = place.clone();
        adj.connect_changed(move |_| place());
    }
    place();

    let drag = gtk::GestureDrag::new();
    {
        let state = state.clone();
        let thumb = thumb.clone();
        drag.connect_drag_begin(move |_, _, _| {
            state.dragging.set(true);
            state.drag_start.set(f64::from(thumb.margin_top()));
        });
    }
    {
        let state = state.clone();
        let thumb = thumb.clone();
        let adj = adj.clone();
        let pads = options.paddings;
        drag.connect_drag_update(move |_, _, dy| {
            let (view, range) = (adj.page_size(), adj.upper());
            let track = track_length(view, pads.top, pads.bottom);
            let y = (state.drag_start.get() + dy).clamp(pads.top, pads.top + track);
            thumb.set_margin_top(y.round() as i32);
            adj.set_value(scroll_for_thumb(y, track, pads.top, range, view));
        });
    }
    {
        let state = state.clone();
        drag.connect_drag_end(move |_, _, _| state.dragging.set(false));
    }
    thumb.add_controller(drag);
}

static THUMB_SEQ: AtomicU32 = AtomicU32::new(0);

/// Gives `thumb` its own colour through a display provider keyed on a class unique to it. The
/// provider is removed when the thumb is destroyed.
fn apply_thumb_color(thumb: &gtk::Box, color: Rgba) {
    let class = format!("kb-thumb-{}", THUMB_SEQ.fetch_add(1, Ordering::Relaxed));
    thumb.add_css_class(&class);
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&crate::theme::style::fast_thumb_color_rule(&class, color));
    let display = thumb.display();
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    thumb.connect_destroy(move |_| gtk::style_context_remove_provider_for_display(&display, &provider));
}

/// Shared by the thumb closures: the drag flag, the drag start and the pending fade timer.
struct ThumbState {
    pads: Paddings,
    dragging: Cell<bool>,
    drag_start: Cell<f64>,
    fade: RefCell<Option<glib::SourceId>>,
}

impl ThumbState {
    fn new(pads: Paddings) -> Self {
        ThumbState {
            pads,
            dragging: Cell::new(false),
            drag_start: Cell::new(0.0),
            fade: RefCell::new(None),
        }
    }
}

/// Shows the thumb now and fades it after the hold (immediately when `allowed` is false). A newer
/// scroll cancels the pending fade. The fade itself is the CSS transition on the shown class.
fn show_then_fade(thumb: &gtk::Box, state: &Rc<ThumbState>, allowed: bool) {
    use crate::theme::style::class::FAST_SHOWN;
    thumb.add_css_class(FAST_SHOWN);
    if let Some(id) = state.fade.borrow_mut().take() {
        id.remove();
    }
    let delay = if allowed { u64::from(SCROLLBAR_FADE_DELAY_MS) } else { 0 };
    let thumb_for_timer = thumb.clone();
    let state_for_timer = state.clone();
    let id = glib::timeout_add_local_once(Duration::from_millis(delay), move || {
        state_for_timer.fade.borrow_mut().take();
        thumb_for_timer.remove_css_class(FAST_SHOWN);
    });
    *state.fade.borrow_mut() = Some(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-49-11: proportional mapping, with clamping at both ends.
    #[test]
    fn scroll_offset_maps_to_the_thumb_and_clamps() {
        let (range, view, track, top) = (1000.0, 200.0, 150.0, 0.0);
        assert_eq!(thumb_top(0.0, range, view, track, top), 0.0);
        assert_eq!(thumb_top(400.0, range, view, track, top), 75.0);
        assert_eq!(thumb_top(800.0, range, view, track, top), 150.0);
        assert_eq!(thumb_top(900.0, range, view, track, top), 150.0);
        assert_eq!(thumb_top(-50.0, range, view, track, top), 0.0);
    }

    #[test]
    fn a_list_that_fits_keeps_the_thumb_at_the_top() {
        assert_eq!(thumb_top(0.0, 150.0, 200.0, 100.0, 4.0), 4.0);
        assert!(!overflows(150.0, 200.0));
        assert!(overflows(900.0, 200.0));
    }

    #[test]
    fn dragging_the_thumb_maps_back_to_scroll_and_clamps_at_the_end() {
        assert_eq!(scroll_for_thumb(75.0, 150.0, 0.0, 1000.0, 200.0), 400.0);
        assert_eq!(scroll_for_thumb(400.0, 150.0, 0.0, 1000.0, 200.0), 800.0);
    }

    // TEST-49-13: grid mapping, 3 columns, 30 items, half way is item 15.
    #[test]
    fn grid_drag_ratio_maps_to_item_index() {
        assert_eq!(item_index_for_ratio(0.5, 30), 15);
        assert_eq!(item_index_for_ratio(1.0, 30), 29);
        assert_eq!(item_index_for_ratio(0.0, 0), 0);
    }

    // TEST-49-12: all visible rows are sticky, so the first row is used, with no panic.
    #[test]
    fn all_sticky_rows_fall_back_to_the_first() {
        let keys = [Some("sticky:a"), Some("sticky:b")];
        assert_eq!(first_non_sticky(&keys), Some(0));
        assert_eq!(first_non_sticky(&[Some("sticky:a"), None]), Some(1));
        assert_eq!(first_non_sticky(&[]), None);
    }

    #[test]
    fn thumb_holds_for_the_delay_then_fades() {
        // T-49a-031: 2 s hold, then 250 ms fade.
        assert_eq!(thumb_opacity(0, 2000, 250), 1.0);
        assert_eq!(thumb_opacity(2000, 2000, 250), 1.0);
        assert!((thumb_opacity(2125, 2000, 250) - 0.5).abs() < 1e-9);
        assert_eq!(thumb_opacity(2300, 2000, 250), 0.0);
        assert_eq!(thumb_opacity(0, 0, 0), 1.0);
        assert_eq!(thumb_opacity(1, 0, 0), 0.0);
    }

    #[test]
    fn thumb_track_is_the_viewport_minus_padding_and_thumb() {
        assert_eq!(track_length(400.0, 4.0, 4.0), 344.0);
        assert_eq!(track_length(40.0, 0.0, 0.0), 0.0);
    }

    // T-49a-029: an unbounded cell is rejected at construction, not turned into a list.
    #[test]
    fn grid_layout_rejects_an_unbounded_cell() {
        assert_eq!(GridLayout::new(0, 8), Err(GridError::UnboundedCell));
        assert_eq!(GridLayout::new(-4, 8), Err(GridError::UnboundedCell));
        assert_eq!(GridLayout::new(128, -1), Err(GridError::NegativeSpacing));
        assert!(GridLayout::new(128, 0).is_ok());
    }

    #[test]
    fn grid_columns_follow_the_width() {
        let layout = GridLayout::new(128, 8).expect("bounded");
        assert_eq!(layout.columns_for(400), 3);
        assert_eq!(layout.columns_for(1000), 7);
        assert_eq!(layout.columns_for(100), 1);
        assert_eq!(layout.columns_for(0), 1);
    }

    #[test]
    fn options_default_to_the_accent_thumb_and_hairline_paddings() {
        let o = Options::new(Mode::Fast);
        assert_eq!(o.mode, Mode::Fast);
        assert_eq!(o.thumb_color, ThumbColor::Accent);
        assert_eq!(o.paddings, Paddings::default());
        assert!((o.thumb_allowed)());
    }
}
