//! Scaffold as a GTK overlay (T-49b-016): the content, the FAB, the snackbar and the bottom bar.
//! The margins come from the pure `layout`, so the rules are tested without a display.

use gtk::prelude::*;

/// The scaffold: the overlay with its FAB and snackbar on top of the content.
pub struct Scaffold {
    pub overlay: gtk::Overlay,
    fab: Option<gtk::Widget>,
    snackbar: gtk::Widget,
    bottom_bar: Option<gtk::Widget>,
}

impl Scaffold {
    /// Builds the overlay. The FAB and the bottom bar are optional.
    pub fn new(
        content: &impl IsA<gtk::Widget>,
        snackbar: &impl IsA<gtk::Widget>,
        fab: Option<&impl IsA<gtk::Widget>>,
        bottom_bar: Option<&impl IsA<gtk::Widget>>,
    ) -> Self {
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(content));
        let snackbar = snackbar.as_ref().clone();
        snackbar.set_halign(gtk::Align::Center);
        snackbar.set_valign(gtk::Align::End);
        overlay.add_overlay(&snackbar);
        let fab = fab.map(|f| {
            let f = f.as_ref().clone();
            f.set_halign(gtk::Align::End);
            f.set_valign(gtk::Align::End);
            overlay.add_overlay(&f);
            f
        });
        let bottom_bar = bottom_bar.map(|b| {
            let b = b.as_ref().clone();
            b.set_valign(gtk::Align::End);
            overlay.add_overlay(&b);
            b
        });
        Scaffold {
            overlay,
            fab,
            snackbar,
            bottom_bar,
        }
    }

}
