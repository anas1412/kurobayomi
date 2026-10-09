//! Theme: colour tokens, scheme resolution, GTK CSS, and the thin widgets built on them.
//!
//! The pure parts (`tokens`, `schemes`, `baseline`, `resolve`, `css`, `dark`, `prefs`, `app_theme`,
//! `style`, `icons`, `empty_state`, `loading`, `cover`) have unit tests and no GTK calls. `apply`,
//! `components` and the widget functions of the other modules are the GTK layer.
//!
//! `startup` is the single call `main.rs` makes before the first window is presented (T-49a-058).
//! The other helpers are used by the screens as their lanes adopt them.
#![allow(dead_code)]

pub mod app_theme;
pub mod apply;
pub mod baseline;
pub mod components;
pub mod cover;
pub mod css;
pub mod dark;
pub mod empty_state;
pub mod icons;
pub mod loading;
pub mod prefs;
pub mod resolve;
mod schemes;
#[cfg(test)]
mod schemes_xml;
pub mod style;
pub mod tokens;
pub mod widget_names;

use kuro_prefs::ui::{AppTheme as Scheme, ThemeMode};

pub use app_theme::AppTheme;
pub use tokens::Tokens;

/// The colour scheme the user picked. `System` leaves the GTK theme's own colours in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemeSel {
    /// Linux default for new installs. Emits no GTK named-colour overrides.
    System,
    /// One of the Aniyomi schemes (Monet resolves to Default until HCT generation lands).
    Scheme(Scheme),
}

impl From<AppTheme> for SchemeSel {
    /// `GtkTheme` and the deprecated names have no scheme, so they select `System`.
    fn from(theme: AppTheme) -> Self {
        match theme.scheme() {
            Some(scheme) => SchemeSel::Scheme(scheme),
            None => SchemeSel::System,
        }
    }
}

/// Persisted theme inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeSettings {
    pub scheme: SchemeSel,
    pub amoled: bool,
    pub mode: ThemeMode,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        ThemeSettings {
            scheme: SchemeSel::System,
            amoled: false,
            mode: ThemeMode::SYSTEM,
        }
    }
}

/// Installs the theme before the first window: the icon bundle, the stylesheets, then the portal
/// follower. Uses the default settings (the GTK theme, system mode); the stored choice joins them
/// once the preference store is reachable from the window lane.
pub fn startup() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    icons::register(&display);
    apply::install(&display, ThemeSettings::default());
    apply::follow_system();
}

/// A resolved palette, ready for CSS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub tokens: Tokens,
    /// True when the choice is `System`: the GTK named colours must not be overridden.
    pub system_scheme: bool,
}

/// Light or dark, from the mode and the system preference. `system_dark` only counts for `SYSTEM`.
pub fn is_dark(mode: ThemeMode, system_dark: bool) -> bool {
    match mode {
        ThemeMode::LIGHT => false,
        ThemeMode::DARK => true,
        ThemeMode::SYSTEM => system_dark,
    }
}

/// Pure entry point: settings and the system dark flag in, tokens out. No GTK calls.
pub fn resolve(settings: &ThemeSettings, system_dark: bool) -> Resolved {
    let dark = is_dark(settings.mode, system_dark);
    let app = match settings.scheme {
        SchemeSel::System => Scheme::DEFAULT,
        SchemeSel::Scheme(app) => app,
    };
    let scheme = resolve::scheme_for(app);
    Resolved {
        tokens: resolve::tokens_for(scheme, dark, settings.amoled),
        system_scheme: settings.scheme == SchemeSel::System,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::tokens::{Role, Rgba};

    #[test]
    fn light_mode_ignores_system_dark() {
        let s = ThemeSettings {
            mode: ThemeMode::LIGHT,
            ..ThemeSettings::default()
        };
        assert!(!resolve(&s, true).tokens.dark);
    }

    #[test]
    fn dark_mode_ignores_system_light() {
        let s = ThemeSettings {
            mode: ThemeMode::DARK,
            ..ThemeSettings::default()
        };
        assert!(resolve(&s, false).tokens.dark);
    }

    #[test]
    fn system_mode_follows_system_dark() {
        let s = ThemeSettings::default();
        assert!(resolve(&s, true).tokens.dark);
        assert!(!resolve(&s, false).tokens.dark);
    }

    #[test]
    fn system_scheme_is_flagged_and_scheme_is_not() {
        let system = resolve(&ThemeSettings::default(), false);
        assert!(system.system_scheme);
        let s = ThemeSettings {
            scheme: SchemeSel::Scheme(Scheme::CLOUDFLARE),
            ..ThemeSettings::default()
        };
        assert!(!resolve(&s, false).system_scheme);
    }

    #[test]
    fn cloudflare_primary_is_its_orange() {
        let s = ThemeSettings {
            scheme: SchemeSel::Scheme(Scheme::CLOUDFLARE),
            mode: ThemeMode::LIGHT,
            ..ThemeSettings::default()
        };
        assert_eq!(resolve(&s, false).tokens.get(Role::Primary), Rgba::argb(0xFFF3_8020));
    }
}
