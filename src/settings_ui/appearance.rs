//! Theme mode (task T-47a-011), the pure-black visibility rule (T-47a-014 gate) and the start screen (T-48a-005).
use kuro_prefs::prefs::Prefs;
use kuro_prefs::ui::{StartScreen, ThemeMode};

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::rows::enum_choice;

/// XDG portal `color-scheme` value for "prefer dark" (0 = no preference, 2 = light).
pub const PORTAL_PREFER_DARK: u32 = 1;

/// Whether the dark theme is in effect. LIGHT and DARK force it; SYSTEM follows the portal, and "no preference" or
/// a missing portal means light (the GTK default).
pub fn effective_dark(mode: ThemeMode, portal_color_scheme: Option<u32>) -> bool {
    match mode {
        ThemeMode::LIGHT => false,
        ThemeMode::DARK => true,
        ThemeMode::SYSTEM => portal_color_scheme == Some(PORTAL_PREFER_DARK),
    }
}

/// The pure-black switch is only offered while the dark theme is in effect.
pub fn amoled_visible(mode: ThemeMode, portal_color_scheme: Option<u32>) -> bool {
    effective_dark(mode, portal_color_scheme)
}

/// Theme mode list: System, Light, Dark. Stored by constant name.
pub fn theme_mode_row(prefs: &Prefs) -> PreferenceItem {
    enum_choice(
        "Theme",
        &prefs.ui.theme_mode,
        &[(ThemeMode::SYSTEM, "System"), (ThemeMode::LIGHT, "Light"), (ThemeMode::DARK, "Dark")],
        true,
    )
}

/// The five start screens in list order, with their titles.
pub const START_SCREENS: [(StartScreen, &str); 5] = [
    (StartScreen::ANIME, "Anime"),
    (StartScreen::MANGA, "Manga"),
    (StartScreen::UPDATES, "Updates"),
    (StartScreen::HISTORY, "History"),
    (StartScreen::BROWSE, "Browse"),
];

pub fn start_screen_row(prefs: &Prefs) -> PreferenceItem {
    enum_choice("Start screen", &prefs.ui.start_screen, &START_SCREENS, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_ignore_the_portal() {
        assert!(!effective_dark(ThemeMode::LIGHT, Some(PORTAL_PREFER_DARK)));
        assert!(effective_dark(ThemeMode::DARK, Some(2)));
        assert!(effective_dark(ThemeMode::DARK, None));
    }

    #[test]
    fn system_follows_the_portal_and_falls_back_to_light() {
        assert!(effective_dark(ThemeMode::SYSTEM, Some(1)));
        assert!(!effective_dark(ThemeMode::SYSTEM, Some(2)));
        assert!(!effective_dark(ThemeMode::SYSTEM, Some(0)));
        assert!(!effective_dark(ThemeMode::SYSTEM, None));
    }

    #[test]
    fn pure_black_is_offered_only_in_dark() {
        assert!(!amoled_visible(ThemeMode::LIGHT, Some(1)));
        assert!(amoled_visible(ThemeMode::SYSTEM, Some(1)));
    }

    #[test]
    fn theme_row_defaults_to_system_and_offers_three_modes() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { value, entries, .. } = theme_mode_row(&prefs) else { panic!("list") };
        assert_eq!(value, "SYSTEM");
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn start_screen_labels_match_the_spec_and_default_is_anime() {
        let labels: Vec<&str> = START_SCREENS.iter().map(|(_, l)| *l).collect();
        assert_eq!(labels, ["Anime", "Manga", "Updates", "History", "Browse"]);
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { value, .. } = start_screen_row(&prefs) else { panic!("list") };
        assert_eq!(value, "ANIME");
    }
}
