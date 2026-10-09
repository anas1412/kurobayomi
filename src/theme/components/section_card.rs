//! Section card (T-49a-035): optional title at a 32 px side inset, then a card with 16/8 px margins
//! and 16 px inner padding.

use gtk::prelude::*;

use super::classes;
use crate::theme::tokens::{PAD_EXTRA_LARGE, PAD_MEDIUM, PAD_SMALL};

/// The card's outer margins as (horizontal, vertical) px.
pub fn card_margins() -> (f32, f32) {
    (PAD_MEDIUM, PAD_SMALL)
}

/// The title's side inset in px.
pub fn title_inset() -> f32 {
    PAD_EXTRA_LARGE
}

/// A section card around `child`, with an optional title above it.
pub fn section_card(title: Option<&str>, child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if let Some(text) = title {
        let label = gtk::Label::new(Some(text));
        label.add_css_class(classes::SECTION_TITLE);
        label.set_xalign(0.0);
        label.set_margin_start(title_inset() as i32);
        label.set_margin_end(title_inset() as i32);
        outer.append(&label);
    }
    let (mh, mv) = card_margins();
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class(classes::SECTION_CARD);
    card.set_margin_start(mh as i32);
    card.set_margin_end(mh as i32);
    card.set_margin_top(mv as i32);
    card.set_margin_bottom(mv as i32);
    card.append(child);
    outer.append(&card);
    outer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_geometry_matches_the_spec() {
        assert_eq!(card_margins(), (16.0, 8.0));
        assert_eq!(title_inset(), 32.0);
    }
}
