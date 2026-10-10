//! Scaffold as a GTK overlay (T-49b-016): the content, the FAB, the snackbar and the bottom bar.
//! The margins come from the pure `layout`, so the rules are tested without a display.

use gtk::prelude::*;

/// Margin between the FAB and the window edges (end and bottom).
pub const FAB_EDGE: i32 = 16;

/// The margins the scaffold applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub fab_bottom: i32,
    pub fab_end: i32,
    pub snackbar_bottom: i32,
}

/// `max(bottomBarHeight, bottomInset)` plus the FAB edge. A hidden FAB reserves no space, so the
/// snackbar then sits on the bar alone.
pub fn layout(bottom_bar_height: i32, bottom_inset: i32, fab_visible: bool) -> Layout {
    let base = bottom_bar_height.max(bottom_inset);
    let fab_bottom = base + FAB_EDGE;
    Layout {
        fab_bottom,
        fab_end: FAB_EDGE,
        snackbar_bottom: if fab_visible { fab_bottom } else { base },
    }
}

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

    /// Re-applies the margins for the current bar height, bottom inset and FAB visibility.
    pub fn relayout(&self, bottom_inset: i32) {
        let bar = self.bottom_bar.as_ref().map_or(0, |b| b.height());
        let fab_visible = self.fab.as_ref().is_some_and(|f| f.is_visible());
        let l = layout(bar, bottom_inset, fab_visible);
        if let Some(fab) = &self.fab {
            fab.set_margin_bottom(l.fab_bottom);
            fab.set_margin_end(l.fab_end);
        }
        self.snackbar.set_margin_bottom(l.snackbar_bottom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_fab_reserves_no_space() {
        let l = layout(56, 0, false);
        assert_eq!(l.snackbar_bottom, 56);
    }

    #[test]
    fn fab_sits_above_the_bar_or_the_inset_whichever_is_taller() {
        assert_eq!(layout(56, 20, true).fab_bottom, 72);
        assert_eq!(layout(56, 80, true).fab_bottom, 96);
        assert_eq!(layout(56, 80, true).fab_end, FAB_EDGE);
    }

    #[test]
    fn snackbar_sits_on_the_fab_when_one_is_visible() {
        let l = layout(56, 0, true);
        assert_eq!(l.snackbar_bottom, l.fab_bottom);
        assert!(l.snackbar_bottom > 56);
    }
}
