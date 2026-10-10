//! The reader bottom bar (T-043a-042): reading mode, rotation, crop and settings. The icons follow the raw stored
//! value (a Default manga shows the Default icon). The icon names are PROPOSALs where the mockup has none.

use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::reader::{ReaderOrientation, ReadingMode};

use super::screen::crop_icon;
use super::strings;

pub const ICON_SETTINGS: &str = "emblem-system-symbolic";

/// One bar button: its accessible description (the Kotlin content description, in English) and its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarButton {
    pub description: &'static str,
    pub icon: &'static str,
}

/// The reading-mode icon for a mode. Default has its own icon, so the raw value is visible.
pub fn mode_icon(mode: ReadingMode) -> &'static str {
    match mode {
        ReadingMode::Default => "view-paged-symbolic",
        ReadingMode::LeftToRight => "go-next-symbolic",
        ReadingMode::RightToLeft => "go-previous-symbolic",
        ReadingMode::Vertical => "go-down-symbolic",
        ReadingMode::Webtoon => "view-list-symbolic",
        ReadingMode::ContinuousVertical => "view-continuous-symbolic",
    }
}

/// The rotation icon for an orientation. Default has its own icon.
pub fn orientation_icon(orientation: ReaderOrientation) -> &'static str {
    match orientation {
        ReaderOrientation::Default => "view-restore-symbolic",
        ReaderOrientation::Free => "object-rotate-right-symbolic",
        ReaderOrientation::Portrait => "object-rotate-left-symbolic",
        ReaderOrientation::Landscape => "object-flip-horizontal-symbolic",
        ReaderOrientation::LockedPortrait | ReaderOrientation::LockedLandscape => "changes-prevent-symbolic",
        ReaderOrientation::ReversePortrait => "object-rotate-right-symbolic",
    }
}

/// The four buttons in order: reading mode, rotation, crop borders, settings.
pub fn bar_buttons(mode: ReadingMode, orientation: ReaderOrientation, crop_on: bool) -> [BarButton; 4] {
    [
        BarButton { description: strings::VIEWER, icon: mode_icon(mode) },
        BarButton { description: strings::ROTATION_TYPE, icon: orientation_icon(orientation) },
        BarButton { description: strings::PREF_CROP_BORDERS, icon: crop_icon(crop_on) },
        BarButton { description: strings::ACTION_SETTINGS, icon: ICON_SETTINGS },
    ]
}

/// The bar. Each callback runs on the GTK thread.
pub struct BottomBar {
    pub root: gtk::Box,
    buttons: [gtk::Button; 4],
}

impl BottomBar {
    pub fn new() -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.set_homogeneous(true);
        root.set_margin_top(8);
        root.set_margin_bottom(8);
        let buttons = [0, 1, 2, 3].map(|_| {
            let b = gtk::Button::new();
            b.add_css_class("flat");
            b.set_hexpand(true);
            root.append(&b);
            b
        });
        Rc::new(Self { root, buttons })
    }

    /// Connects the four buttons: reading mode, rotation, crop borders and settings.
    pub fn connect(
        &self,
        on_mode: impl Fn() + 'static,
        on_rotation: impl Fn() + 'static,
        on_crop: impl Fn() + 'static,
        on_settings: impl Fn() + 'static,
    ) {
        self.buttons[0].connect_clicked(move |_| on_mode());
        self.buttons[1].connect_clicked(move |_| on_rotation());
        self.buttons[2].connect_clicked(move |_| on_crop());
        self.buttons[3].connect_clicked(move |_| on_settings());
    }

    /// Shows the raw mode and orientation, and the crop state as a toggled button.
    pub fn update(&self, mode: ReadingMode, orientation: ReaderOrientation, crop_on: bool) {
        for (button, spec) in self.buttons.iter().zip(bar_buttons(mode, orientation, crop_on)) {
            button.set_icon_name(spec.icon);
            button.set_tooltip_text(Some(spec.description));
            button.update_property(&[gtk::accessible::Property::Label(spec.description)]);
        }
        let classes: &[&str] = if crop_on { &["flat", "toggled"] } else { &["flat"] };
        self.buttons[2].set_css_classes(classes);
        // The crop button is a toggle: its pressed state is exposed as well as the icon change (T-43a-089).
        let pressed = if crop_on { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False };
        self.buttons[2].update_state(&[gtk::accessible::State::Pressed(pressed)]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_buttons_in_the_spec_order() {
        let b = bar_buttons(ReadingMode::Webtoon, ReaderOrientation::Free, true);
        let names: Vec<&str> = b.iter().map(|x| x.description).collect();
        assert_eq!(names, ["Reading mode", "Rotation", "Crop borders", "Settings"]);
    }

    #[test]
    fn the_default_mode_and_orientation_show_their_own_icons() {
        assert_eq!(mode_icon(ReadingMode::Default), "view-paged-symbolic");
        assert_ne!(mode_icon(ReadingMode::Default), mode_icon(ReadingMode::Webtoon));
        assert_eq!(orientation_icon(ReaderOrientation::Default), "view-restore-symbolic");
        assert_ne!(orientation_icon(ReaderOrientation::Default), orientation_icon(ReaderOrientation::Free));
    }

    #[test]
    fn the_crop_icon_follows_the_crop_state() {
        assert_eq!(bar_buttons(ReadingMode::Default, ReaderOrientation::Default, true)[2].icon, crop_icon(true));
        assert_eq!(bar_buttons(ReadingMode::Default, ReaderOrientation::Default, false)[2].icon, crop_icon(false));
    }
}
