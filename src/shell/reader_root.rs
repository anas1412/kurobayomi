//! Reader window root layout (T-53a-057): the viewer under a page number pill (bottom centre), a navigation overlay that
//! is hidden by default and takes no input, and a dialog host above the viewer for reader dialogs.

use gtk::prelude::*;

use crate::reader::indicator::indicator_text;

/// The reader root. `overlay` holds the viewer as its child; the other three are overlays on top of it.
pub struct ReaderRoot {
    pub overlay: gtk::Overlay,
    pub pill: gtk::Label,
    pub navigation: gtk::Box,
    pub dialogs: gtk::Box,
}

impl ReaderRoot {
    /// Shows `page / total` in the pill, or hides it when the indicator text is `None`.
    pub fn set_page(&self, page: i32, total: i32, menu_visible: bool, show_page_number: bool) {
        match indicator_text(page, total, menu_visible, show_page_number) {
            Some(text) => {
                self.pill.set_text(&text);
                self.pill.set_visible(true);
            }
            None => self.pill.set_visible(false),
        }
    }
}

/// Builds the layout around `viewer`.
pub fn build(viewer: &impl IsA<gtk::Widget>) -> ReaderRoot {
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(viewer));

    let pill = gtk::Label::builder().css_classes(["osd"]).visible(false).halign(gtk::Align::Center).valign(gtk::Align::End).margin_bottom(24).build();
    overlay.add_overlay(&pill);

    let navigation = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    navigation.set_visible(false);
    navigation.set_can_target(false);
    navigation.set_can_focus(false);
    overlay.add_overlay(&navigation);

    let dialogs = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dialogs.set_hexpand(true);
    dialogs.set_vexpand(true);
    dialogs.set_can_target(false);
    overlay.add_overlay(&dialogs);

    ReaderRoot { overlay, pill, navigation, dialogs }
}
