//! The Linux theme picker's `AppTheme` (Aniyomi `AppTheme.kt`): stored names, picker order, titles,
//! and the colour scheme each theme resolves to (T-49b-043, T-49ba-002).
//!
//! Deviations from the Kotlin enum, all Linux decisions:
//! - `GtkTheme` is added. It follows the GTK theme (no palette CSS) and is the default for new
//!   installs and for any unknown stored value.
//! - The stored names of the deprecated `DARK_BLUE`, `HOT_PINK` and `BLUE` still parse, and they
//!   resolve to `GtkTheme`. They are never offered and have no title.
//! - There is no dynamic (`MONET`) choice. Its stored name is not known here, so it reads as `GtkTheme`
//!   like any unknown value.

use kuro_i18n::{MsgKey, tr};
use kuro_prefs::ui::AppTheme as Scheme;

/// One theme choice. The declaration order matches `AppTheme.kt`, with `GtkTheme` first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppTheme {
    GtkTheme,
    Default,
    Cloudflare,
    CottonCandy,
    Doom,
    GreenApple,
    Lavender,
    Matrix,
    MidnightDusk,
    Mocha,
    Sapphire,
    Nord,
    StrawberryDaiquiri,
    Tako,
    TealTurquoise,
    TidalWave,
    YinYang,
    Yotsuba,
    Monochrome,
    /// Kurobayomi's addition (T-49a-063). Not in `AppTheme.kt`; it has no Android name.
    TokyoNight,
    DarkBlue,
    HotPink,
    Blue,
}

impl AppTheme {
    /// Every variant, including the deprecated ones kept for parsing.
    pub const ALL: [AppTheme; 23] = [
        AppTheme::GtkTheme,
        AppTheme::Default,
        AppTheme::Cloudflare,
        AppTheme::CottonCandy,
        AppTheme::Doom,
        AppTheme::GreenApple,
        AppTheme::Lavender,
        AppTheme::Matrix,
        AppTheme::MidnightDusk,
        AppTheme::Mocha,
        AppTheme::Sapphire,
        AppTheme::Nord,
        AppTheme::StrawberryDaiquiri,
        AppTheme::Tako,
        AppTheme::TealTurquoise,
        AppTheme::TidalWave,
        AppTheme::YinYang,
        AppTheme::Yotsuba,
        AppTheme::Monochrome,
        AppTheme::TokyoNight,
        AppTheme::DarkBlue,
        AppTheme::HotPink,
        AppTheme::Blue,
    ];

    /// The picker order: `GtkTheme`, then the 18 selectable Android themes, then Tokyo Night.
    pub const PICKER: [AppTheme; 20] = [
        AppTheme::GtkTheme,
        AppTheme::Default,
        AppTheme::Cloudflare,
        AppTheme::CottonCandy,
        AppTheme::Doom,
        AppTheme::GreenApple,
        AppTheme::Lavender,
        AppTheme::Matrix,
        AppTheme::MidnightDusk,
        AppTheme::Mocha,
        AppTheme::Sapphire,
        AppTheme::Nord,
        AppTheme::StrawberryDaiquiri,
        AppTheme::Tako,
        AppTheme::TealTurquoise,
        AppTheme::TidalWave,
        AppTheme::YinYang,
        AppTheme::Yotsuba,
        AppTheme::Monochrome,
        AppTheme::TokyoNight,
    ];

    /// The stored name: `SYSTEM` for `GtkTheme` (the value `theme::prefs` writes on first run and the specs show), otherwise
    /// the Android constant name.
    pub const fn stored_name(self) -> &'static str {
        match self {
            AppTheme::GtkTheme => "SYSTEM",
            AppTheme::Default => "DEFAULT",
            AppTheme::Cloudflare => "CLOUDFLARE",
            AppTheme::CottonCandy => "COTTONCANDY",
            AppTheme::Doom => "DOOM",
            AppTheme::GreenApple => "GREEN_APPLE",
            AppTheme::Lavender => "LAVENDER",
            AppTheme::Matrix => "MATRIX",
            AppTheme::MidnightDusk => "MIDNIGHT_DUSK",
            AppTheme::Mocha => "MOCHA",
            AppTheme::Sapphire => "SAPPHIRE",
            AppTheme::Nord => "NORD",
            AppTheme::StrawberryDaiquiri => "STRAWBERRY_DAIQUIRI",
            AppTheme::Tako => "TAKO",
            AppTheme::TealTurquoise => "TEALTURQUOISE",
            AppTheme::TidalWave => "TIDAL_WAVE",
            AppTheme::YinYang => "YINYANG",
            AppTheme::Yotsuba => "YOTSUBA",
            AppTheme::Monochrome => "MONOCHROME",
            AppTheme::TokyoNight => "TOKYO_NIGHT",
            AppTheme::DarkBlue => "DARK_BLUE",
            AppTheme::HotPink => "HOT_PINK",
            AppTheme::Blue => "BLUE",
        }
    }

    /// The variant with this stored name, or `None` for an unknown string. Matching is exact:
    /// `mocha` and the empty string are unknown.
    pub fn parse(name: &str) -> Option<AppTheme> {
        AppTheme::ALL.into_iter().find(|t| t.stored_name() == name)
    }

    /// The theme a stored value selects. Unknown values fall back to `GtkTheme`, the Linux default.
    pub fn from_stored(name: &str) -> AppTheme {
        AppTheme::parse(name).unwrap_or(AppTheme::GtkTheme)
    }

    /// True for the three deprecated names, which are never offered.
    pub const fn is_deprecated(self) -> bool {
        matches!(self, AppTheme::DarkBlue | AppTheme::HotPink | AppTheme::Blue)
    }

    /// The theme that is actually applied: deprecated names become `GtkTheme`.
    pub const fn effective(self) -> AppTheme {
        if self.is_deprecated() { AppTheme::GtkTheme } else { self }
    }

    /// True when a stored value must be written back. Unknown values are never written back
    /// (they stay until the user picks a theme); deprecated names are migrated to `SYSTEM`.
    pub fn needs_write_back(stored: &str) -> bool {
        AppTheme::parse(stored).is_some_and(AppTheme::is_deprecated)
    }

    /// The picker title, from the translation catalogue. `None` for the deprecated names.
    pub fn title(self) -> Option<String> {
        let key = match self {
            // The Linux default follows the desktop theme; the catalogue has no key of its own.
            AppTheme::GtkTheme => MsgKey::theme_system,
            AppTheme::Default => MsgKey::label_default,
            AppTheme::Cloudflare => MsgKey::theme_cloudflare,
            AppTheme::CottonCandy => MsgKey::theme_cottoncandy,
            AppTheme::Doom => MsgKey::theme_doom,
            AppTheme::GreenApple => MsgKey::theme_greenapple,
            AppTheme::Lavender => MsgKey::theme_lavender,
            AppTheme::Matrix => MsgKey::theme_matrix,
            AppTheme::MidnightDusk => MsgKey::theme_midnightdusk,
            AppTheme::Mocha => MsgKey::theme_mocha,
            AppTheme::Sapphire => MsgKey::theme_sapphire,
            AppTheme::Nord => MsgKey::theme_nord,
            AppTheme::StrawberryDaiquiri => MsgKey::theme_strawberrydaiquiri,
            AppTheme::Tako => MsgKey::theme_tako,
            AppTheme::TealTurquoise => MsgKey::theme_tealturquoise,
            AppTheme::TidalWave => MsgKey::theme_tidalwave,
            AppTheme::YinYang => MsgKey::theme_yinyang,
            AppTheme::Yotsuba => MsgKey::theme_yotsuba,
            AppTheme::Monochrome => MsgKey::theme_monochrome,
            // A proper name, so it has no catalogue key.
            AppTheme::TokyoNight => return Some("Tokyo Night".to_string()),
            AppTheme::DarkBlue | AppTheme::HotPink | AppTheme::Blue => return None,
        };
        Some(tr(key))
    }

    /// The colour scheme this theme uses. `None` means no palette: `GtkTheme`, Tokyo Night (its own
    /// scheme) and the deprecated names.
    pub fn scheme(self) -> Option<Scheme> {
        Some(match self {
            AppTheme::GtkTheme | AppTheme::TokyoNight | AppTheme::DarkBlue | AppTheme::HotPink | AppTheme::Blue => {
                return None;
            }
            AppTheme::Default => Scheme::DEFAULT,
            AppTheme::Cloudflare => Scheme::CLOUDFLARE,
            AppTheme::CottonCandy => Scheme::COTTONCANDY,
            AppTheme::Doom => Scheme::DOOM,
            AppTheme::GreenApple => Scheme::GREEN_APPLE,
            AppTheme::Lavender => Scheme::LAVENDER,
            AppTheme::Matrix => Scheme::MATRIX,
            AppTheme::MidnightDusk => Scheme::MIDNIGHT_DUSK,
            AppTheme::Mocha => Scheme::MOCHA,
            AppTheme::Sapphire => Scheme::SAPPHIRE,
            AppTheme::Nord => Scheme::NORD,
            AppTheme::StrawberryDaiquiri => Scheme::STRAWBERRY_DAIQUIRI,
            AppTheme::Tako => Scheme::TAKO,
            AppTheme::TealTurquoise => Scheme::TEALTURQUOISE,
            AppTheme::TidalWave => Scheme::TIDAL_WAVE,
            AppTheme::YinYang => Scheme::YINYANG,
            AppTheme::Yotsuba => Scheme::YOTSUBA,
            AppTheme::Monochrome => Scheme::MONOCHROME,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::SchemeSel;

    #[test]
    fn picker_lists_gtk_theme_first_then_eighteen_schemes_then_tokyo_night() {
        assert_eq!(AppTheme::PICKER.len(), 20);
        assert_eq!(AppTheme::PICKER[0], AppTheme::GtkTheme);
        assert_eq!(AppTheme::PICKER[1], AppTheme::Default);
        assert_eq!(AppTheme::PICKER[18], AppTheme::Monochrome);
        assert_eq!(AppTheme::PICKER[19], AppTheme::TokyoNight);
    }

    // T-49a-063: an addition, stored as TOKYO_NIGHT, with no Aniyomi scheme of its own.
    #[test]
    fn tokyo_night_is_stored_and_titled_without_an_aniyomi_scheme() {
        assert_eq!(AppTheme::from_stored("TOKYO_NIGHT"), AppTheme::TokyoNight);
        assert_eq!(AppTheme::TokyoNight.title().as_deref(), Some("Tokyo Night"));
        assert_eq!(AppTheme::TokyoNight.scheme(), None);
        assert_eq!(SchemeSel::from(AppTheme::TokyoNight), SchemeSel::TokyoNight);
    }

    #[test]
    fn every_selectable_theme_has_a_scheme_or_is_gtk_theme() {
        for theme in AppTheme::PICKER {
            if theme != AppTheme::GtkTheme && theme != AppTheme::TokyoNight {
                assert!(theme.scheme().is_some(), "{theme:?}");
            }
        }
        assert_eq!(AppTheme::GtkTheme.scheme(), None);
    }

    #[test]
    fn stored_names_round_trip_for_every_variant() {
        for theme in AppTheme::ALL {
            assert_eq!(AppTheme::parse(theme.stored_name()), Some(theme));
        }
        assert_eq!(AppTheme::Cloudflare.stored_name(), "CLOUDFLARE");
        assert_eq!(AppTheme::GtkTheme.stored_name(), "SYSTEM", "the name theme::prefs writes");
        assert_eq!(AppTheme::GtkTheme.stored_name(), crate::theme::prefs::SYSTEM_NAME);
    }

    #[test]
    fn unknown_values_fall_back_without_panicking() {
        for name in ["", "mocha", "NOT_A_THEME"] {
            assert_eq!(AppTheme::parse(name), None);
            assert_eq!(AppTheme::from_stored(name), AppTheme::GtkTheme);
            assert!(!AppTheme::needs_write_back(name), "unknown {name:?} is not written back");
        }
    }

    #[test]
    fn deprecated_names_parse_but_resolve_to_gtk_theme_and_have_no_title() {
        for name in ["DARK_BLUE", "HOT_PINK", "BLUE"] {
            let theme = AppTheme::from_stored(name);
            assert!(theme.is_deprecated());
            assert_eq!(theme.effective(), AppTheme::GtkTheme);
            assert_eq!(theme.title(), None);
            assert_eq!(theme.scheme(), None);
            assert!(AppTheme::needs_write_back(name));
        }
    }

    #[test]
    fn titles_come_from_the_catalogue() {
        assert_eq!(AppTheme::Default.title().as_deref(), Some("Default"));
        assert_eq!(AppTheme::Cloudflare.title().as_deref(), Some("Cloudflare"));
        assert!(AppTheme::Monochrome.title().is_some());
    }

    #[test]
    fn mocha_and_cotton_candy_keep_their_schemes() {
        assert_eq!(AppTheme::from_stored("MOCHA").scheme(), Some(Scheme::MOCHA));
        assert_eq!(AppTheme::from_stored("COTTONCANDY").scheme(), Some(Scheme::COTTONCANDY));
    }
}

#[cfg(test)]
mod monet_removed_tests {
    use super::AppTheme;

    #[test]
    fn a_stored_monet_is_unknown_here_and_reads_as_the_gtk_theme() {
        assert_eq!(AppTheme::parse("MONET"), None);
        assert_eq!(AppTheme::from_stored("MONET"), AppTheme::GtkTheme);
    }
}
