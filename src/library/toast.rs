//! The library's toast (U-1.8, T-37a-062): one message at a time at the bottom of the library page, hidden after 4 s.
//!
//! Decision: the shared toast layer (`crate::toast`) only knows 2 s and 3.5 s, and neither it nor `kuro_platform::toast`
//! belongs to the library lane, so the library presents its own message on an overlay around its view. A message
//! replaces the one on screen; a click dismisses it. When no library page is on screen (a message that arrives while
//! another page covers the library) the shared layer shows it instead.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

/// How long a library toast stays (U-1.8).
pub const HIDE_AFTER: Duration = Duration::from_secs(4);

/// Counts the messages shown. A hide timer only hides the message it was started for, so a newer message keeps its full
/// time.
#[derive(Debug, Default)]
pub struct Generation(Cell<u64>);

impl Generation {
    /// A new message is shown. Returns its number.
    pub fn next(&self) -> u64 {
        self.0.set(self.0.get() + 1);
        self.0.get()
    }

    /// Whether `mine` is still the message on screen.
    pub fn is_current(&self, mine: u64) -> bool {
        self.0.get() == mine
    }

    /// Ends the current message: no pending timer matches it any more.
    pub fn dismiss(&self) {
        self.next();
    }
}

struct Presenter {
    label: gtk::Label,
    revealer: gtk::Revealer,
    generation: Generation,
}

impl Presenter {
    fn show(self: &Rc<Self>, text: &str) {
        let mine = self.generation.next();
        self.label.set_text(text);
        self.revealer.set_reveal_child(true);
        let this = Rc::downgrade(self);
        glib::timeout_add_local_once(HIDE_AFTER, move || {
            if let Some(this) = this.upgrade() {
                if this.generation.is_current(mine) {
                    this.revealer.set_reveal_child(false);
                }
            }
        });
    }

    fn dismiss(&self) {
        self.generation.dismiss();
        self.revealer.set_reveal_child(false);
    }
}

thread_local! {
    static PRESENTERS: RefCell<Vec<Weak<Presenter>>> = const { RefCell::new(Vec::new()) };
}

/// Wraps `content` in an overlay that carries the library toast. Call from the GTK main thread.
pub fn attach(content: &impl IsA<gtk::Widget>) -> gtk::Overlay {
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
    let presenter = Rc::new(Presenter { label: label.clone(), revealer: revealer.clone(), generation: Generation::default() });
    let click = gtk::GestureClick::new();
    let weak = Rc::downgrade(&presenter);
    click.connect_pressed(move |_, _, _, _| {
        if let Some(presenter) = weak.upgrade() {
            presenter.dismiss();
        }
    });
    label.add_controller(click);

    let overlay = gtk::Overlay::new();
    overlay.set_hexpand(true);
    overlay.set_vexpand(true);
    overlay.set_child(Some(content));
    overlay.add_overlay(&revealer);
    PRESENTERS.with(|p| {
        let mut p = p.borrow_mut();
        p.retain(|w| w.strong_count() > 0);
        p.push(Rc::downgrade(&presenter));
    });
    // the presenter lives as long as the overlay does
    let keep = RefCell::new(Some(presenter));
    overlay.connect_destroy(move |_| drop(keep.borrow_mut().take()));
    overlay
}

/// Shows `text` on the library page that is on screen, for 4 s. Without one, the shared toast layer shows it.
pub fn notify(text: impl Into<String>) {
    let text = text.into();
    let on_screen = PRESENTERS.with(|p| p.borrow().iter().filter_map(Weak::upgrade).find(|p| p.revealer.is_mapped()));
    match on_screen {
        Some(presenter) => presenter.show(&text),
        None => crate::toast::notify(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_toast_hides_after_four_seconds() {
        assert_eq!(HIDE_AFTER, Duration::from_millis(4000));
    }

    #[test]
    fn a_newer_message_keeps_its_own_time() {
        let generation = Generation::default();
        let first = generation.next();
        let second = generation.next();
        assert!(!generation.is_current(first), "the first timer must not hide the second message");
        assert!(generation.is_current(second));
    }

    #[test]
    fn a_dismissed_message_is_not_hidden_again_by_its_timer() {
        let generation = Generation::default();
        let shown = generation.next();
        generation.dismiss();
        assert!(!generation.is_current(shown));
    }
}
