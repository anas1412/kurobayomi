//! The app theme choice: System, Light or Dark. The colour palettes (the Aniyomi names of `pref_app_theme`, Tokyo Night
//! included) were cut by the settings diet, so any stored palette name reads as System.
//!
//! The choice is stored as the mode name in `pref_theme_mode_key` (`SYSTEM`, `LIGHT`, `DARK`).

use kuro_prefs::ui::ThemeMode;

/// One theme choice, in picker order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppTheme {
    System,
    Light,
    Dark,
}

impl AppTheme {
    #[cfg(test)]
    /// The picker order.
    pub const PICKER: [AppTheme; 3] = [AppTheme::System, AppTheme::Light, AppTheme::Dark];

    /// The stored mode this choice writes.
    pub const fn mode(self) -> ThemeMode {
        match self {
            AppTheme::System => ThemeMode::SYSTEM,
            AppTheme::Light => ThemeMode::LIGHT,
            AppTheme::Dark => ThemeMode::DARK,
        }
    }

    /// The choice a stored value selects. Unknown values, and the cut palette names, fall back to System.
    pub fn from_stored(name: &str) -> AppTheme {
        match ThemeMode::from_name(name) {
            Some(ThemeMode::LIGHT) => AppTheme::Light,
            Some(ThemeMode::DARK) => AppTheme::Dark,
            _ => AppTheme::System,
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_lists_system_light_and_dark() {
        assert_eq!(AppTheme::PICKER, [AppTheme::System, AppTheme::Light, AppTheme::Dark]);
    }

    #[test]
    fn stored_mode_names_map_to_their_choice() {
        assert_eq!(AppTheme::from_stored("LIGHT"), AppTheme::Light);
        assert_eq!(AppTheme::from_stored("DARK"), AppTheme::Dark);
        assert_eq!(AppTheme::from_stored("SYSTEM"), AppTheme::System);
        for choice in AppTheme::PICKER {
            assert_eq!(AppTheme::from_stored(choice.mode().name()), choice);
        }
    }

    #[test]
    fn palette_names_and_unknown_values_fall_back_to_system() {
        for name in ["", "mocha", "NOT_A_THEME", "CLOUDFLARE", "TOKYO_NIGHT", "MONET", "HOT_PINK"] {
            assert_eq!(AppTheme::from_stored(name), AppTheme::System, "{name:?}");
        }
    }
}
