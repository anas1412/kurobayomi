//! A toast that stays for a given time at the bottom of one screen (T-42a-016, T-42a-024). The app's own toast layer
//! has fixed lengths, and the History rules name 4 seconds, so the screens keep this one in an overlay.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

pub struct ScreenToast {
    label: gtk::Label,
    /// Counts shown toasts; a timer only hides the toast it was started for.
    generation: Rc<Cell<u64>>,
}

impl Default for ScreenToast {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenToast {
    pub fn new() -> Self {
        let label = gtk::Label::builder()
            .wrap(true)
            .max_width_chars(60)
            .css_classes(["osd"])
            .halign(gtk::Align::Center)
            .valign(gtk::Align::End)
            .margin_bottom(24)
            .visible(false)
            .can_target(false)
            .focusable(false)
            .build();
        Self { label, generation: Rc::new(Cell::new(0)) }
    }

    /// The label to add as an overlay of the screen.
    pub fn widget(&self) -> &gtk::Label {
        &self.label
    }

    /// Shows `text` for `ms` milliseconds. A newer toast replaces it and restarts the time.
    pub fn show(&self, text: &str, ms: u64) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.label.set_label(text);
        self.label.set_visible(true);
        let (label, current) = (self.label.clone(), Rc::clone(&self.generation));
        glib::timeout_add_local_once(Duration::from_millis(ms), move || {
            if current.get() == generation {
                label.set_visible(false);
            }
        });
    }

    pub fn is_showing(&self) -> bool {
        self.label.is_visible()
    }
}
