//! Collapsible box (T-49a-026) built on `gtk::Expander`. Collapsed by default; the arrow swaps between
//! up and down; Enter and Space toggle it through the expander's own keys.

use gtk::prelude::*;

use crate::theme::style::class;

/// Header padding in px: 24 horizontal, 12 vertical.
pub const HEADER_PAD_H: i32 = 24;
pub const HEADER_PAD_V: i32 = 12;

/// Whether the box starts open. It starts collapsed.
pub const START_EXPANDED: bool = false;

/// A collapsible box with `title` as its header and `content` as its body.
pub fn collapsible(title: &str, content: &impl IsA<gtk::Widget>) -> gtk::Expander {
    let expander = gtk::Expander::new(Some(title));
    expander.set_expanded(START_EXPANDED);
    expander.set_child(Some(content));
    expander.add_css_class(class::COLLAPSIBLE);
    expander
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_collapsed_with_the_source_header_padding() {
        assert!(!START_EXPANDED);
        assert_eq!((HEADER_PAD_H, HEADER_PAD_V), (24, 12));
    }
}
