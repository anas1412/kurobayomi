//! Extended floating action button (T-49a-052): a 56 px round-cornered button with an icon, and a
//! label that expands to the right. Collapsed is 56 px wide; expanded is at least 80 px.
//!
//! Timings are the source's (`p1-behavior.md` 22), all in `theme::style::widget_static_rules`:
//! width 500 ms, start padding 300 ms to expand and 900 ms to collapse, label fade 200 ms (100 ms
//! delay) to expand and 100 ms to collapse. Each is a CSS transition keyed on the expanded class, so
//! a toggle mid-animation restarts from the current value (E-49-34). The revealer only gives the
//! label its horizontal expand and shrink (500 ms); the fade is the label's own opacity.

use gtk::prelude::*;

use crate::theme::tokens::{FAB_COLLAPSED_WIDTH, FAB_EXPANDED_MIN_WIDTH, FAB_ICON_PAD, FAB_TEXT_PAD};

pub const HEIGHT: i32 = 56;

/// Minimum width for a state, in px.
pub fn min_width(expanded: bool) -> i32 {
    if expanded { FAB_EXPANDED_MIN_WIDTH as i32 } else { FAB_COLLAPSED_WIDTH as i32 }
}

/// Label padding `(start, end)` when expanded.
pub fn label_padding() -> (i32, i32) {
    (FAB_ICON_PAD as i32, FAB_TEXT_PAD as i32)
}

/// The FAB handle: the button, and the revealer that holds the label.
pub struct ExtendedFab {
    pub button: gtk::Button,
    revealer: gtk::Revealer,
}

impl ExtendedFab {
    /// Expands or collapses the button. The width change is the CSS transition on the class.
    pub fn set_expanded(&self, expanded: bool) {
        self.revealer.set_reveal_child(expanded);
        if expanded {
            self.button.add_css_class(crate::theme::style::class::FAB_EXPANDED);
        } else {
            self.button.remove_css_class(crate::theme::style::class::FAB_EXPANDED);
        }
    }
}

/// Builds the extended FAB with `label`, `icon_name`, starting expanded or not.
pub fn extended_fab(label: &str, icon_name: &str, expanded: bool) -> ExtendedFab {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_valign(gtk::Align::Center);
    row.add_css_class(crate::theme::style::class::FAB_ROW);
    let icon = gtk::Image::from_icon_name(icon_name);
    row.append(&icon);

    let text = gtk::Label::new(Some(label));
    text.add_css_class(crate::theme::style::class::FAB_LABEL);
    let (start, end) = label_padding();
    text.set_margin_start(start);
    text.set_margin_end(end);
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideRight);
    revealer.set_transition_duration(crate::theme::motion::duration_ms(
        crate::theme::tokens::FAB_WIDTH_MS,
        crate::theme::motion::enabled(),
    ));
    revealer.set_child(Some(&text));
    row.append(&revealer);

    let button = gtk::Button::new();
    button.add_css_class(crate::theme::style::class::FAB);
    button.set_child(Some(&row));
    button.set_size_request(min_width(false), HEIGHT);
    let fab = ExtendedFab { button, revealer };
    fab.set_expanded(expanded);
    fab
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::tokens::{
        FAB_COLLAPSE_FADE_MS, FAB_EXPAND_FADE_DELAY_MS, FAB_EXPAND_FADE_MS, FAB_START_PAD_COLLAPSE_MS,
        FAB_START_PAD_EXPAND_MS, FAB_WIDTH_MS,
    };

    // TEST-49-21: the constants equal p1-behavior.md section 22.
    #[test]
    fn timings_match_the_source() {
        assert_eq!(FAB_WIDTH_MS, 500);
        assert_eq!(FAB_START_PAD_EXPAND_MS, 300);
        assert_eq!(FAB_START_PAD_COLLAPSE_MS, 900);
        assert_eq!(FAB_EXPAND_FADE_MS, 200);
        assert_eq!(FAB_EXPAND_FADE_DELAY_MS, 100);
        assert_eq!(FAB_COLLAPSE_FADE_MS, 100);
    }

    #[test]
    fn widths_and_label_padding() {
        assert_eq!(min_width(false), 56);
        assert_eq!(min_width(true), 80);
        assert_eq!(label_padding(), (12, 20));
    }
}
