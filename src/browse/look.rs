//! The look the Updates, History and Browse tabs share with the Library page (docs/design/other-tabs.md), beside the
//! shared column, heading and status of `ui`: worded badges, the empty state with its one action, and the covers.
//! Stock GTK widgets and theme colours only.

use gtk::prelude::*;
use gtk::{Align, Orientation as O};
use kuro_images::art::ArtRef;

use crate::library::layout::{self, CoverKind, SPACE_L};

/// A badge that says what it counts (`3 updates`, `Installed`). `accent` takes the theme's selected colour.
pub fn badge(text: &str, accent: bool) -> gtk::Label {
    let label = gtk::Label::builder().label(text).valign(Align::Center).css_classes(["kb-badge"]).build();
    if accent {
        label.add_css_class("kb-badge-unread");
    }
    label
}

/// The empty state of a page (`ui::status`: a large dim icon over the message), with at most one action under it.
/// Returns the page and its message label, for pages that change the message.
pub fn empty_state(icon: &str, message: &str, action: Option<&gtk::Button>) -> (gtk::Box, gtk::Label) {
    let page = crate::ui::status(icon, message);
    page.set_margin_start(SPACE_L);
    page.set_margin_end(SPACE_L);
    let label = page.last_child().and_downcast::<gtk::Label>().expect("the status ends with its message");
    if let Some(action) = action {
        action.set_halign(Align::Center);
        page.append(action);
    }
    (page, label)
}

/// The cover of a list row: [`cover`] with the small rounded corners and initials of a row.
pub fn row_cover(title: &str, art: Option<&ArtRef>, width: i32, height: i32) -> gtk::Widget {
    let cover = cover(title, art, width, height);
    cover.add_css_class("kb-row-cover");
    cover.set_overflow(gtk::Overflow::Hidden);
    cover.set_valign(Align::Center);
    cover
}

/// A cover: the stored image when it can be drawn, else the title's initials on a tile, as the library cards do.
/// Always `width` x `height`.
pub fn cover(title: &str, art: Option<&ArtRef>, width: i32, height: i32) -> gtk::Widget {
    let remote = crate::entry_core::cover_view::service().is_some();
    let cover: gtk::Widget = match layout::cover_kind(art.and_then(|a| a.url.as_deref()), remote) {
        CoverKind::File(url) => crate::local_flow::cover::cover_widget(Some(&url), width, height).upcast(),
        CoverKind::Remote(_) => {
            let view = crate::entry_core::cover_view::CoverView::new(width, height);
            if let Some(art) = art {
                view.load(art.clone());
            }
            view.frame.upcast()
        }
        CoverKind::Initials => {
            let text = gtk::Label::builder().label(layout::initials(title)).vexpand(true).valign(Align::Center).css_classes(["kb-cover-initials-text"]).build();
            let tile = gtk::Box::new(O::Vertical, 0);
            tile.add_css_class("kb-cover-initials");
            tile.set_size_request(width, height);
            tile.append(&text);
            tile.upcast()
        }
    };
    cover
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_without_a_stored_cover_shows_the_initials_and_the_empty_state_has_one_action() {
        crate::testing::with_display(|| {
            let cover = row_cover("Solo Leveling", None, 40, 56);
            assert!(cover.has_css_class("kb-cover-initials") && cover.has_css_class("kb-row-cover"));
            let text = cover.first_child().and_downcast::<gtk::Label>().expect("the initials");
            assert_eq!(text.label(), "SL");

            let action = gtk::Button::with_label("Update library");
            let (page, label) = empty_state("view-refresh-symbolic", "No recent updates", Some(&action));
            assert_eq!(label.label(), "No recent updates");
            assert!(label.has_css_class("kb-empty-title"));
            assert_eq!(action.parent().as_ref(), Some(page.upcast_ref::<gtk::Widget>()));
        });
    }
}
