//! GTK components built on the theme classes. Pure helpers are unit-tested; the builders are thin.

pub mod badge;
pub mod dialog;
pub mod progress_ring;
pub mod section_card;
pub mod settings_row;

/// CSS class names. Rules for each live in `css::component_rules`.
pub mod classes {
    pub const BADGE: &str = "kuro-badge";
    pub const BADGE_DOWNLOADED: &str = "downloaded";
    pub const BADGE_GROUP: &str = "kuro-badge-group";
    pub const PILL: &str = "kuro-pill";
    pub const GROUP_HEADER: &str = "kuro-group-header";
    pub const SECTION_CARD: &str = "kuro-section-card";
    pub const SECTION_TITLE: &str = "kuro-section-title";
    pub const DIALOG: &str = "kuro-dialog";
    pub const DIALOG_TITLE: &str = "kuro-dialog-title";
    pub const DIALOG_TEXT: &str = "kuro-dialog-text";
    pub const PROGRESS_RING: &str = "kuro-progress-ring";
    pub const SETTINGS_ROW: &str = "kuro-settings-row";
    pub const SETTINGS_HEADING: &str = "kuro-settings-heading";
    pub const SETTINGS_ICON: &str = "kuro-settings-icon";
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
    fn class_names_are_kuro_prefixed_except_the_badge_variant() {
        for name in [
            classes::BADGE,
            classes::BADGE_GROUP,
            classes::PILL,
            classes::GROUP_HEADER,
            classes::SECTION_CARD,
            classes::DIALOG,
            classes::PROGRESS_RING,
            classes::SETTINGS_ROW,
        ] {
            assert!(name.starts_with("kuro-"), "{name}");
        }
        assert_eq!(classes::BADGE_DOWNLOADED, "downloaded");
    }
}
