//! The page indicator (T-043a-049): "n / total" at the bottom centre, shown only with the menu hidden and the
//! page number preference on.

use gtk::prelude::*;

/// The indicator text, or `None` when it is hidden: menu visible, preference off, or a page or total not above zero.
pub fn indicator_text(page: i32, total: i32, menu_visible: bool, show_page_number: bool) -> Option<String> {
    if menu_visible || !show_page_number || page <= 0 || total <= 0 {
        None
    } else {
        Some(format!("{page} / {total}"))
    }
}

/// The label. Its look (bold, 235-grey text with a dark outline) is in the reader stylesheet (`theme::css`).
pub struct PageIndicator {
    pub label: gtk::Label,
}

impl PageIndicator {
    pub fn new() -> Self {
        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Center);
        label.set_valign(gtk::Align::End);
        label.set_margin_bottom(8);
        label.add_css_class("reader-page-indicator");
        label.set_visible(false);
        Self { label }
    }

    pub fn update(&self, page: i32, total: i32, menu_visible: bool, show_page_number: bool) {
        match indicator_text(page, total, menu_visible, show_page_number) {
            Some(text) => {
                self.label.set_label(&text);
                self.label.set_visible(true);
            }
            None => self.label.set_visible(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_indicator_shows_only_with_the_menu_hidden_and_the_preference_on() {
        assert_eq!(indicator_text(3, 12, false, true).as_deref(), Some("3 / 12"));
        assert_eq!(indicator_text(3, 12, true, true), None);
        assert_eq!(indicator_text(3, 12, false, false), None);
    }

    #[test]
    fn a_page_or_total_that_is_not_above_zero_hides_it() {
        assert_eq!(indicator_text(0, 12, false, true), None);
        assert_eq!(indicator_text(-1, 12, false, true), None);
        assert_eq!(indicator_text(1, 0, false, true), None);
    }
}
