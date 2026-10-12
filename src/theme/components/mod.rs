//! GTK components built on the theme classes. Pure helpers are unit-tested; the builders are thin.

/// CSS class names. Rules for each live in `css::component_rules`.
pub mod classes {
    pub const PILL: &str = "kuro-pill";
    pub const GROUP_HEADER: &str = "kuro-group-header";
    pub const CHECK_DISABLED: &str = "kuro-check";
}

use gtk::prelude::*;

/// A list group header: body medium, semibold, `on_surface_variant`, padding 16/8 (T-49a-033).
pub fn group_header(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class(classes::GROUP_HEADER);
    label.set_xalign(0.0);
    label
}

#[cfg(test)]
mod tests {
    use super::classes;

    #[test]
    fn class_names_are_kuro_prefixed() {
        for name in [classes::PILL, classes::GROUP_HEADER] {
            assert!(name.starts_with("kuro-"), "{name}");
        }
    }
}
