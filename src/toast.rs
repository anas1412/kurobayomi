//! The toast overlay (T-12a-014 presenter). `kuro_platform::toast` queues the messages; this shows them one at a
//! time at the bottom of the main window, each for its length. GTK objects stay on the main thread.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use kuro_platform::toast::{ToastFeed, Toaster, toast_channel};

// A new message replaces the one on screen. Clicking the toast or pressing Escape dismisses it.
//
// Manual verification (no display in CI): the toast never takes keyboard focus. The label is neither
// focusable nor focus-targetable and the Escape handler is a capture-phase controller that grabs nothing, so
// with a text entry focused, showing a toast leaves the caret in the entry and typing still goes to it.

static GLOBAL: std::sync::OnceLock<Toaster> = std::sync::OnceLock::new();

thread_local! {
    /// The overlay that [`busy`] adds its spinner card to (set by [`attach`]).
    static OVERLAY: std::cell::RefCell<Option<gtk::Overlay>> = const { std::cell::RefCell::new(None) };
}

/// A centred spinner card with `text` over the main window until the returned guard is dropped: the "Loading..."
/// state of an action that waits for the network. Without an overlay (tests without a window) it does nothing.
pub fn busy(text: &str) -> Busy {
    let card = OVERLAY.with(|o| o.borrow().clone()).map(|overlay| {
        let spinner = gtk::Spinner::builder().spinning(true).width_request(32).height_request(32).build();
        let label = gtk::Label::new(Some(text));
        let row = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(12).margin_top(14).margin_bottom(14).margin_start(18).margin_end(18).build();
        row.append(&spinner);
        row.append(&label);
        let card = gtk::Frame::builder().child(&row).halign(gtk::Align::Center).valign(gtk::Align::Center).css_classes(["app-notification", "osd"]).can_target(false).build();
        overlay.add_overlay(&card);
        (overlay, card)
    });
    Busy(card)
}

/// Removes the spinner card of [`busy`] when dropped.
pub struct Busy(Option<(gtk::Overlay, gtk::Frame)>);

impl Drop for Busy {
    fn drop(&mut self) {
        if let Some((overlay, card)) = self.0.take() {
            overlay.remove_overlay(&card);
        }
    }
}

/// Short toast through the overlay attached by [`attach`]; dropped when no overlay exists yet.
pub fn notify(text: impl Into<String>) {
    if let Some(t) = GLOBAL.get() {
        t.show(text, kuro_platform::toast::ToastLength::Short);
    }
}

/// The toast layer: a label and an action button in one bar, shown by [`Layer::show`]. One per window ([`attach`]).
struct Layer {
    label: gtk::Label,
    action: gtk::Button,
    /// What the action button runs now; replaced with each toast.
    on_action: RefCell<Option<Rc<dyn Fn()>>>,
    revealer: gtk::Revealer,
    /// Counts shown toasts; a timer only hides the toast it was started for.
    generation: Cell<u64>,
}

thread_local! {
    static LAYER: RefCell<Option<Rc<Layer>>> = const { RefCell::new(None) };
}

impl Layer {
    fn dismiss(&self) {
        self.generation.set(self.generation.get() + 1);
        self.revealer.set_reveal_child(false);
    }

    /// Shows `text`, replacing the toast on screen. `duration` None keeps it until dismissed or replaced.
    fn show(self: &Rc<Self>, text: &str, duration: Option<Duration>, action: Option<(&str, Rc<dyn Fn()>)>) {
        let mine = self.generation.get() + 1;
        self.generation.set(mine);
        self.label.set_text(text);
        match action {
            Some((label, run)) => {
                self.action.set_label(label);
                self.action.set_visible(true);
                *self.on_action.borrow_mut() = Some(run);
            }
            None => {
                self.action.set_visible(false);
                *self.on_action.borrow_mut() = None;
            }
        }
        self.revealer.set_reveal_child(true);
        if let Some(duration) = duration {
            let this = Rc::downgrade(self);
            glib::timeout_add_local_once(duration, move || {
                if let Some(this) = this.upgrade() {
                    if this.generation.get() == mine {
                        this.revealer.set_reveal_child(false);
                    }
                }
            });
        }
    }
}

/// How long a toast with an action stays: long enough to reach for Undo or Retry.
pub const ACTION_DURATION: Duration = Duration::from_secs(5);

/// A toast for `duration` instead of the fixed 2 s. Must run on the GTK main thread; dropped when no layer is attached.
pub fn notify_for(text: &str, duration: Duration) {
    if let Some(layer) = LAYER.with(|l| l.borrow().clone()) {
        layer.show(text, Some(duration), None);
    }
}

/// A toast with one action button (Undo, Retry): `run` runs when it is pressed, and the toast goes away. `duration`
/// None keeps the toast until the user dismisses it (a click on the text or Escape) or another toast replaces it.
/// Must run on the GTK main thread; dropped when no layer is attached.
pub fn notify_action(text: &str, action_label: &str, duration: Option<Duration>, run: impl Fn() + 'static) {
    if let Some(layer) = LAYER.with(|l| l.borrow().clone()) {
        let run: Rc<dyn Fn()> = Rc::new(run);
        layer.show(text, duration, Some((action_label, run)));
    }
}

/// Adds the toast layer to `overlay` and starts the presenter on the main context. Returns the sender that
/// producers use. Call once, from the GTK main thread.
pub fn attach(overlay: &gtk::Overlay) -> Toaster {
    let (toaster, feed) = toast_channel();
    let label = gtk::Label::builder().wrap(true).max_width_chars(60).hexpand(true).xalign(0.0).focusable(false).can_focus(false).build();
    let action = gtk::Button::builder().css_classes(["flat"]).visible(false).focusable(false).can_focus(false).focus_on_click(false).build();
    let bar = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(6).css_classes(["osd"]).margin_bottom(24).margin_start(12).margin_end(12).build();
    bar.append(&label);
    bar.append(&action);
    let revealer = gtk::Revealer::builder()
        .child(&bar)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::End)
        .transition_type(gtk::RevealerTransitionType::SlideUp)
        .focusable(false)
        .can_focus(false)
        .build();
    let layer = Rc::new(Layer { label: label.clone(), action: action.clone(), on_action: RefCell::new(None), revealer: revealer.clone(), generation: Cell::new(0) });

    let click = gtk::GestureClick::new();
    click.connect_pressed({
        let layer = layer.clone();
        move |_, _, _, _| layer.dismiss()
    });
    label.add_controller(click);

    action.connect_clicked({
        let layer = layer.clone();
        move |_| {
            let run = layer.on_action.borrow_mut().take();
            layer.dismiss();
            if let Some(run) = run {
                run();
            }
        }
    });

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let layer = layer.clone();
        move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape && layer.revealer.reveals_child() {
                layer.dismiss();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    overlay.add_controller(keys);

    overlay.add_overlay(&revealer);
    OVERLAY.with(|o| *o.borrow_mut() = Some(overlay.clone()));
    LAYER.with(|l| *l.borrow_mut() = Some(layer.clone()));
    glib::spawn_future_local(present(feed, layer, toaster.clone()));
    let _ = GLOBAL.set(toaster.clone());
    toaster
}

/// `keep_open` holds a sender for as long as the presenter runs, so the feed stays open even when the
/// returned handle is dropped.
async fn present(feed: ToastFeed, layer: Rc<Layer>, keep_open: Toaster) {
    while let Some(message) = feed.next().await {
        layer.show(&message.text, Some(message.length.duration()), None);
    }
    drop(keep_open);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_platform::toast::ToastLength;

    /// The first widget below `root` that is a `T`.
    fn find<T: IsA<gtk::Widget>>(root: &gtk::Widget) -> Option<T> {
        if let Some(found) = root.downcast_ref::<T>() {
            return Some(found.clone());
        }
        let mut child = root.first_child();
        while let Some(c) = child {
            if let Some(found) = find::<T>(&c) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }

    fn pump(ms: u64) {
        let until = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        while std::time::Instant::now() < until {
            glib::MainContext::default().iteration(false);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    // T-47b-052, E73: one toast at a time in a Revealer over the overlay; a new one replaces the visible one and
    // the last one hides after its 2 s. No libadwaita widget is involved.
    #[test]
    fn a_new_toast_replaces_the_visible_one_and_the_last_hides_after_two_seconds() {
        crate::testing::with_display(|| {
            assert_eq!(ToastLength::Short.duration(), std::time::Duration::from_secs(2));
            let overlay = gtk::Overlay::new();
            let toaster = attach(&overlay);
            let revealer: gtk::Revealer = find(overlay.upcast_ref()).expect("the toast revealer");
            let label: gtk::Label = find(revealer.upcast_ref()).expect("the toast label");
            assert!(!revealer.reveals_child(), "nothing shows before the first toast");
            assert_eq!(revealer.halign(), gtk::Align::Center);
            assert_eq!(revealer.valign(), gtk::Align::End, "bottom centre");

            toaster.show("first", ToastLength::Short);
            pump(1000);
            assert!(revealer.reveals_child());
            assert_eq!(label.text(), "first");
            toaster.show("second", ToastLength::Short);
            pump(100);
            assert_eq!(label.text(), "second", "a new toast replaces the visible one");

            // the first toast's 2 s run out here; they must not hide the second one
            pump(1200);
            assert!(revealer.reveals_child(), "the first toast's timer does not hide the second");
            pump(1200);
            assert!(!revealer.reveals_child(), "hidden 2 s after the last toast");
        });
    }
}
