//! Text and icon badges (T-49a-024). Default colours are secondary; the downloaded variant is tertiary.

use gtk::prelude::*;

use super::classes;

/// Which colour pair a badge uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeKind {
    Default,
    Downloaded,
}

/// The CSS classes for a badge kind. The base class is always first.
pub fn badge_classes(kind: BadgeKind) -> Vec<&'static str> {
    match kind {
        BadgeKind::Default => vec![classes::BADGE],
        BadgeKind::Downloaded => vec![classes::BADGE, classes::BADGE_DOWNLOADED],
    }
}

/// Icon size inside a badge, in px (T-49a-024: 12 px).
pub const BADGE_ICON_SIZE: i32 = 12;

/// A one-line text badge; long text ends with an ellipsis (E-49-37).
pub fn text_badge(text: &str, kind: BadgeKind) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    for class in badge_classes(kind) {
        label.add_css_class(class);
    }
    label.set_single_line_mode(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label
}

/// An icon badge: the icon at 12 px with the same colours as the text badge.
pub fn icon_badge(icon_name: &str, kind: BadgeKind) -> gtk::Image {
    let image = gtk::Image::from_icon_name(icon_name);
    for class in badge_classes(kind) {
        image.add_css_class(class);
    }
    image.set_pixel_size(BADGE_ICON_SIZE);
    image
}

/// A horizontal group of badges with a small gap.
pub fn badge_group(children: &[gtk::Widget]) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.add_css_class(classes::BADGE_GROUP);
    for child in children {
        row.append(child);
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_badge_uses_the_base_class_only() {
        assert_eq!(badge_classes(BadgeKind::Default), vec!["kuro-badge"]);
    }

    #[test]
    fn downloaded_badge_adds_the_tertiary_variant() {
        assert_eq!(
            badge_classes(BadgeKind::Downloaded),
            vec!["kuro-badge", "downloaded"]
        );
    }

    #[test]
    fn icon_size_is_twelve_px() {
        assert_eq!(BADGE_ICON_SIZE, 12);
    }
}
