//! The category tab strip (T-37a-016): one toggle button per visible page, in a horizontal scroller.

use std::rc::Rc;

use gtk::Orientation as O;
use gtk::prelude::*;

#[derive(Clone)]
pub struct TabStrip {
    pub scroller: gtk::ScrolledWindow,
    row: gtk::Box,
}

impl TabStrip {
    pub fn new() -> Self {
        let row = gtk::Box::new(O::Horizontal, 4);
        row.set_margin_start(12);
        row.set_margin_end(12);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&row)
            .build();
        Self { scroller, row }
    }

    /// Rebuilds the strip with `labels`, `active` pressed. `on_select` gets the index of a tab the user picks.
    /// The buttons are created before their handlers are connected, so setting the active one does not fire it.
    pub fn set(&self, labels: &[String], active: usize, on_select: Rc<dyn Fn(usize)>) {
        while let Some(child) = self.row.first_child() {
            self.row.remove(&child);
        }
        let mut group: Option<gtk::ToggleButton> = None;
        for (index, text) in labels.iter().enumerate() {
            let button = gtk::ToggleButton::with_label(text);
            match &group {
                Some(first) => button.set_group(Some(first)),
                None => group = Some(button.clone()),
            }
            button.set_active(index == active);
            let on_select = on_select.clone();
            button.connect_toggled(move |b| {
                if b.is_active() {
                    on_select(index);
                }
            });
            self.row.append(&button);
        }
        self.scroller.set_visible(!labels.is_empty());
    }
}
