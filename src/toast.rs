//! The toast overlay (T-12a-014 presenter). `kuro_platform::toast` queues the messages; this shows them one at a
//! time at the bottom of the main window, each for its length. GTK objects stay on the main thread.

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use kuro_platform::toast::{ToastFeed, Toaster, ToastMessage, toast_channel};

// A new message replaces the one on screen. Clicking the toast or pressing Escape dismisses it.
//
// Manual verification (no display in CI): the toast never takes keyboard focus. The label is neither
// focusable nor focus-targetable and the Escape handler is a capture-phase controller that grabs nothing, so
// with a text entry focused, showing a toast leaves the caret in the entry and typing still goes to it.

static GLOBAL: std::sync::OnceLock<Toaster> = std::sync::OnceLock::new();

/// Short toast through the overlay attached by [`attach`]; dropped when no overlay exists yet.
pub fn notify(text: impl Into<String>) {
    if let Some(t) = GLOBAL.get() {
        t.show(text, kuro_platform::toast::ToastLength::Short);
    }
}

/// Adds the toast layer to `overlay` and starts the presenter on the main context. Returns the sender that
/// producers use. Call once, from the GTK main thread.
pub fn attach(overlay: &gtk::Overlay) -> Toaster {
    let (toaster, feed) = toast_channel();
    let label = gtk::Label::builder()
        .wrap(true)
        .max_width_chars(60)
        .css_classes(["osd"])
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .focusable(false)
        .can_focus(false)
        .build();
    let revealer = gtk::Revealer::builder()
        .child(&label)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::End)
        .transition_type(gtk::RevealerTransitionType::SlideUp)
        .focusable(false)
        .can_focus(false)
        .build();
    // Counts shown toasts; a timer only hides the toast it was started for.
    let generation = Rc::new(Cell::new(0_u64));

    let click = gtk::GestureClick::new();
    click.connect_pressed({
        let (revealer, generation) = (revealer.clone(), generation.clone());
        move |_, _, _, _| dismiss(&revealer, &generation)
    });
    label.add_controller(click);

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let (revealer, generation) = (revealer.clone(), generation.clone());
        move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape && revealer.reveals_child() {
                dismiss(&revealer, &generation);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    overlay.add_controller(keys);

    overlay.add_overlay(&revealer);
    glib::spawn_future_local(present(feed, label, revealer, generation, toaster.clone()));
    let _ = GLOBAL.set(toaster.clone());
    toaster
}

fn dismiss(revealer: &gtk::Revealer, generation: &Cell<u64>) {
    generation.set(generation.get() + 1);
    revealer.set_reveal_child(false);
}

fn show(message: ToastMessage, label: &gtk::Label, revealer: &gtk::Revealer, generation: &Rc<Cell<u64>>) {
    let mine = generation.get() + 1;
    generation.set(mine);
    label.set_text(&message.text);
    revealer.set_reveal_child(true);
    let (revealer, generation) = (revealer.clone(), generation.clone());
    glib::timeout_add_local_once(message.length.duration(), move || {
        if generation.get() == mine {
            revealer.set_reveal_child(false);
        }
    });
}

/// `keep_open` holds a sender for as long as the presenter runs, so the feed stays open even when the
/// returned handle is dropped.
async fn present(
    feed: ToastFeed,
    label: gtk::Label,
    revealer: gtk::Revealer,
    generation: Rc<Cell<u64>>,
    keep_open: Toaster,
) {
    while let Some(message) = feed.next().await {
        show(message, &label, &revealer, &generation);
    }
    drop(keep_open);
}
