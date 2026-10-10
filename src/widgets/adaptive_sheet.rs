//! AdaptiveSheet (T-49a-023): a centred modal on wide windows, a bottom sheet on narrow ones.
//!
//! Both branches are a layer added to the host `gtk::Overlay`: an invisible catcher over the whole
//! host (a tap or click outside the panel dismisses; no scrim is drawn, as in the source) and the
//! panel. Tablet: the panel is centred, 600 px wide in landscape and 460 px in portrait, and fades in
//! and out over 350 ms. Phone: the panel slides up from the bottom over 350 ms, and is dismissed by
//! a downward drag past 56 dp or a fling faster than 125 dp/s, only when swipe dismiss is enabled.
//! `adaptive_sheet` picks the branch from the window size class. Both trap focus (the other overlay
//! children stop taking focus and Tab wraps inside the panel), and Escape closes. The dismissal runs
//! once, and a close after the host is gone does nothing (R-49-03).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

use super::keyboard::{close_on_escape, trap_tab_focus};
use crate::theme::size_class::WindowClass;
use crate::theme::style::class::{SHEET, SHEET_SHOWN, SHEET_TABLET};

/// The sheet slide time: instant under reduced motion (T-49a-057).
fn sheet_anim_ms() -> u32 {
    crate::theme::motion::duration_ms(SHEET_ANIM_MS, crate::theme::motion::enabled())
}

use crate::theme::tokens::{SHEET_ANIM_MS, SHEET_MAX_WIDTH_LANDSCAPE, SHEET_MAX_WIDTH_PORTRAIT, SHEET_POSITIONAL_THRESHOLD, SHEET_VELOCITY_THRESHOLD};

/// Which sheet to build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branch {
    /// Centred modal panel with a fade.
    Tablet,
    /// Bottom sheet with a slide.
    Phone,
}

/// Compact windows get the bottom sheet; Medium and Expanded get the centred panel.
pub fn branch_for(class: WindowClass) -> Branch {
    match class {
        WindowClass::Compact => Branch::Phone,
        WindowClass::Medium | WindowClass::Expanded => Branch::Tablet,
    }
}

/// Maximum sheet width in px for the orientation.
pub fn max_width(landscape: bool) -> i32 {
    if landscape { SHEET_MAX_WIDTH_LANDSCAPE as i32 } else { SHEET_MAX_WIDTH_PORTRAIT as i32 }
}

/// True when a released drag closes the phone sheet. `offset_dp` is how far it was dragged down,
/// `velocity_dp_s` the release speed. Nothing closes when swipe dismiss is disabled (E-49-28).
pub fn should_dismiss(offset_dp: f32, velocity_dp_s: f32, swipe_enabled: bool) -> bool {
    swipe_enabled && (offset_dp > SHEET_POSITIONAL_THRESHOLD || velocity_dp_s > SHEET_VELOCITY_THRESHOLD)
}

/// Runs a dismissal once (E-49-31). The first call returns true.
#[derive(Default)]
pub struct DismissOnce(Cell<bool>);

impl DismissOnce {
    pub fn fire(&self) -> bool {
        !self.0.replace(true)
    }
}

/// The sheet. Build it with `adaptive_sheet`, then `show`. Clones share one sheet.
#[derive(Clone)]
pub struct AdaptiveSheet(Rc<Inner>);

struct Inner {
    host: gtk::Overlay,
    layer: gtk::Overlay,
    panel: gtk::Box,
    /// Phone only: the revealer that slides the panel.
    revealer: Option<gtk::Revealer>,
    once: DismissOnce,
    on_dismiss: Box<dyn Fn()>,
    shown: Cell<bool>,
    /// Undoes the focus trap and gives focus back.
    release: RefCell<Option<Box<dyn FnOnce()>>>,
}

/// Builds the sheet for the current window size class around `child`, for the overlay `host`.
/// `enable_swipe` only matters for the phone branch. `on_dismiss` runs once, when dismissal starts.
pub fn adaptive_sheet(
    host: &gtk::Overlay,
    child: &impl IsA<gtk::Widget>,
    enable_swipe: bool,
    on_dismiss: impl Fn() + 'static,
) -> AdaptiveSheet {
    let branch = branch_for(crate::theme::window_class());
    // An unallocated host (0 x 0) counts as landscape.
    let landscape = host.width() >= host.height();

    let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
    panel.add_css_class(SHEET);
    panel.append(child);

    let catcher = gtk::Box::new(gtk::Orientation::Vertical, 0);
    catcher.set_hexpand(true);
    catcher.set_vexpand(true);
    let layer = gtk::Overlay::new();
    layer.set_child(Some(&catcher));

    let revealer = match branch {
        Branch::Tablet => {
            panel.add_css_class(SHEET_TABLET);
            panel.set_width_request(max_width(landscape));
            panel.set_halign(gtk::Align::Center);
            panel.set_valign(gtk::Align::Center);
            layer.add_overlay(&panel);
            None
        }
        Branch::Phone => {
            panel.set_width_request(max_width(false));
            let revealer = gtk::Revealer::new();
            revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
            revealer.set_transition_duration(sheet_anim_ms());
            revealer.set_valign(gtk::Align::End);
            revealer.set_halign(gtk::Align::Center);
            revealer.set_child(Some(&panel));
            layer.add_overlay(&revealer);
            Some(revealer)
        }
    };

    let sheet = AdaptiveSheet(Rc::new(Inner {
        host: host.clone(),
        layer: layer.clone(),
        panel: panel.clone(),
        revealer,
        once: DismissOnce::default(),
        on_dismiss: Box::new(on_dismiss),
        shown: Cell::new(false),
        release: RefCell::new(None),
    }));

    let dismiss = {
        let weak: Weak<Inner> = Rc::downgrade(&sheet.0);
        Rc::new(move || {
            if let Some(inner) = weak.upgrade() {
                AdaptiveSheet(inner).dismiss();
            }
        })
    };
    let tap = gtk::GestureClick::new();
    {
        let dismiss = dismiss.clone();
        tap.connect_pressed(move |_, _, _, _| dismiss());
    }
    catcher.add_controller(tap);
    {
        let dismiss = dismiss.clone();
        close_on_escape(&layer, move || dismiss());
    }
    trap_tab_focus(&layer);
    if branch == Branch::Phone && enable_swipe {
        attach_swipe(&panel, dismiss);
    }
    sheet
}

impl AdaptiveSheet {
    /// Which branch was built.
    pub fn branch(&self) -> Branch {
        if self.0.revealer.is_some() { Branch::Phone } else { Branch::Tablet }
    }

    /// Adds the layer to the host, traps focus in the panel and starts the fade or the slide.
    pub fn show(&self) {
        let inner = &self.0;
        if inner.shown.replace(true) {
            return;
        }
        inner.host.add_overlay(&inner.layer);
        inner.host.set_measure_overlay(&inner.layer, false);
        let previous = inner.host.root().and_then(|root| root.focus());
        let untrap = trap_focus(&inner.host, &inner.layer);
        let root = inner.host.root();
        *inner.release.borrow_mut() = Some(Box::new(move || {
            untrap();
            if let (Some(root), Some(previous)) = (root, previous) {
                root.set_focus(Some(&previous));
            }
        }));
        let weak = Rc::downgrade(&self.0);
        // The classes and focus are set on the next idle so the first frame starts from the hidden
        // state and the transition has something to run from.
        glib::idle_add_local_once(move || {
            let Some(inner) = weak.upgrade() else { return };
            if !inner.shown.get() {
                return;
            }
            match &inner.revealer {
                Some(revealer) => revealer.set_reveal_child(true),
                None => inner.panel.add_css_class(SHEET_SHOWN),
            }
            if !inner.panel.child_focus(gtk::DirectionType::TabForward) {
                inner.panel.set_focusable(true);
                inner.panel.grab_focus();
            }
        });
    }

    /// Fades or slides the sheet out, runs `on_dismiss` once, then removes the layer and gives focus
    /// back. Does nothing before `show`, or when the host is gone.
    pub fn dismiss(&self) {
        let inner = &self.0;
        if !inner.shown.get() {
            return;
        }
        if inner.once.fire() {
            (inner.on_dismiss)();
        }
        match &inner.revealer {
            Some(revealer) => revealer.set_reveal_child(false),
            None => inner.panel.remove_css_class(SHEET_SHOWN),
        }
        let this = self.clone();
        glib::timeout_add_local_once(Duration::from_millis(u64::from(sheet_anim_ms())), move || this.finish());
    }

    fn finish(&self) {
        let inner = &self.0;
        if !inner.shown.replace(false) {
            return;
        }
        if inner.layer.parent().is_some_and(|p| &p == inner.host.upcast_ref::<gtk::Widget>()) {
            inner.host.remove_overlay(&inner.layer);
        }
        if let Some(release) = inner.release.borrow_mut().take() {
            release();
        }
    }
}

/// Stops every other child of `host` (the main child and the other overlays) from taking focus, and
/// returns the undo. `can-focus` covers the whole subtree in GTK 4.
fn trap_focus(host: &gtk::Overlay, layer: &gtk::Overlay) -> Box<dyn FnOnce()> {
    let mut saved = Vec::new();
    let mut child = host.first_child();
    while let Some(widget) = child {
        if &widget != layer.upcast_ref::<gtk::Widget>() {
            saved.push((widget.clone(), widget.can_focus()));
            widget.set_can_focus(false);
        }
        child = widget.next_sibling();
    }
    Box::new(move || {
        for (widget, was) in saved {
            widget.set_can_focus(was);
        }
    })
}

/// Follows a downward drag on the sheet and dismisses it or springs back on release. Velocity is
/// estimated from the last two drag updates.
fn attach_swipe(panel: &gtk::Box, dismiss: Rc<dyn Fn()>) {
    let drag = gtk::GestureDrag::new();
    let last = Rc::new(Cell::new((0i64, 0.0f64)));
    let velocity = Rc::new(Cell::new(0.0f64));
    let weak = panel.downgrade();
    {
        let (last, velocity) = (last.clone(), velocity.clone());
        drag.connect_drag_begin(move |_, _, _| {
            last.set((glib::monotonic_time(), 0.0));
            velocity.set(0.0);
        });
    }
    {
        let (last, velocity, weak) = (last.clone(), velocity.clone(), weak.clone());
        drag.connect_drag_update(move |_, _, dy| {
            let dy = dy.max(0.0);
            let now = glib::monotonic_time();
            let (then, was) = last.get();
            if now > then {
                velocity.set((dy - was) / ((now - then) as f64 / 1_000_000.0));
            }
            last.set((now, dy));
            if let Some(panel) = weak.upgrade() {
                panel.set_margin_top(dy as i32);
            }
        });
    }
    drag.connect_drag_end(move |_, _, dy| {
        let offset = dy.max(0.0) as f32;
        if should_dismiss(offset, velocity.get() as f32, true) {
            dismiss();
        } else if let Some(panel) = weak.upgrade() {
            panel.set_margin_top(0);
        }
    });
    panel.add_controller(drag);
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-49-19
    #[test]
    fn drag_and_fling_thresholds() {
        assert!(!should_dismiss(55.0, 0.0, true));
        assert!(should_dismiss(57.0, 0.0, true));
        assert!(should_dismiss(0.0, 130.0, true));
        assert!(!should_dismiss(0.0, 120.0, true));
    }

    #[test]
    fn disabled_swipe_never_dismisses_by_drag() {
        // E-49-28
        assert!(!should_dismiss(500.0, 500.0, false));
    }

    // TEST-49-20 / E-49-31
    #[test]
    fn dismissal_fires_once() {
        let once = DismissOnce::default();
        assert!(once.fire());
        assert!(!once.fire());
        assert!(!once.fire());
    }

    // T-49a-023: compact windows get the bottom sheet, wider ones the centred panel.
    #[test]
    fn size_class_picks_the_branch() {
        assert_eq!(branch_for(WindowClass::Compact), Branch::Phone);
        assert_eq!(branch_for(WindowClass::Medium), Branch::Tablet);
        assert_eq!(branch_for(WindowClass::Expanded), Branch::Tablet);
    }

    #[test]
    fn width_depends_on_orientation() {
        assert_eq!(max_width(true), 600);
        assert_eq!(max_width(false), 460);
    }
}
