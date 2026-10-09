//! Theme mode selector (Kotlin `AppThemeModePreferenceWidget`): System, Light, Dark as a linked toggle group.
//! The stored value is the enum name. System leaves the colour choice to the desktop.

use gtk::prelude::*;
use gtk::{Box as GBox, Orientation, ToggleButton};
use kuro_prefs::pref::Pref;
use kuro_prefs::ui::{apply_theme, ThemeChoice, ThemeMode};

use super::super::model::ItemCommon;
use super::row::{self, PrefRow, RowSpec};

/// Segment order in the UI (the enum declaration order differs).
pub const SEGMENTS: [ThemeMode; 3] = [ThemeMode::SYSTEM, ThemeMode::LIGHT, ThemeMode::DARK];

pub fn segment_label(mode: ThemeMode) -> String {
    use kuro_i18n::{MsgKey, tr};
    tr(match mode {
        ThemeMode::SYSTEM => MsgKey::theme_system,
        ThemeMode::LIGHT => MsgKey::theme_light,
        ThemeMode::DARK => MsgKey::theme_dark,
    })
}

/// A click on the selected segment does not write (E-50).
pub fn should_write(current: ThemeMode, next: ThemeMode) -> bool {
    current != next
}

/// Pushes the choice to GTK. System resets the property so the desktop decides again (P-theme-dark).
pub fn apply(mode: ThemeMode) {
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    match apply_theme(mode) {
        ThemeChoice::Light => settings.set_gtk_application_prefer_dark_theme(false),
        ThemeChoice::Dark => settings.set_gtk_application_prefer_dark_theme(true),
        ThemeChoice::FollowSystem => settings.reset_property("gtk-application-prefer-dark-theme"),
    }
}

pub fn row(common: &ItemCommon, pref: Pref<ThemeMode>) -> PrefRow {
    let group = GBox::new(Orientation::Horizontal, 0);
    group.add_css_class("linked");
    group.set_valign(gtk::Align::Center);
    let current = pref.get();
    apply(current);
    let mut buttons: Vec<(ThemeMode, ToggleButton)> = Vec::new();
    for mode in SEGMENTS {
        let button = ToggleButton::with_label(&segment_label(mode));
        if let Some((_, first)) = buttons.first() {
            button.set_group(Some(first));
        }
        button.set_active(mode == current);
        let pref = pref.clone();
        button.connect_toggled(move |b| {
            if !b.is_active() || !should_write(pref.get(), mode) {
                return;
            }
            if pref.set(mode).is_ok() {
                apply(mode);
            }
        });
        group.append(&button);
        buttons.push((mode, button));
    }
    let built = row::build(RowSpec {
        title: &common.title,
        subtitle: None,
        icon: common.icon,
        trailing: Some(group.upcast_ref()),
        on_click: None,
    });
    let weak = group.downgrade();
    super::super::bind::follow(&pref, move || weak.upgrade().is_some(), move |mode: ThemeMode| {
        apply(mode);
        if let Some((_, button)) = buttons.iter().find(|(m, _)| *m == mode) {
            button.set_active(true);
        }
    });
    built
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_comes_first_and_the_same_value_is_not_written() {
        assert_eq!(SEGMENTS, [ThemeMode::SYSTEM, ThemeMode::LIGHT, ThemeMode::DARK]);
        assert!(!should_write(ThemeMode::DARK, ThemeMode::DARK));
        assert!(should_write(ThemeMode::DARK, ThemeMode::SYSTEM));
    }

    #[test]
    fn system_passes_the_follow_system_choice_not_a_forced_colour() {
        assert_eq!(apply_theme(ThemeMode::SYSTEM), ThemeChoice::FollowSystem);
        assert_eq!(apply_theme(ThemeMode::LIGHT), ThemeChoice::Light);
        assert_eq!(apply_theme(ThemeMode::DARK), ThemeChoice::Dark);
    }
}
