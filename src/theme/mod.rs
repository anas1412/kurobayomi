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
pub mod elevation;
pub mod empty_state;
pub mod icons;
pub mod loading;
pub mod monet;
pub mod picker;
pub mod prefs;
pub mod resolve;
pub mod size_class;
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
    /// One of the Aniyomi schemes. `Scheme(MONET)` is the dynamic choice (see `monet`).
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

thread_local! {
    /// The window's size class, updated by `attach_window`. `Compact` until the first layout.
    static WINDOW_CLASS: std::cell::Cell<size_class::WindowClass> =
        const { std::cell::Cell::new(size_class::WindowClass::Compact) };
}

/// The current window size class (T-49a-021). Read by the layouts that branch on it.
pub fn window_class() -> size_class::WindowClass {
    WINDOW_CLASS.with(std::cell::Cell::get)
}

/// Installs the theme before the first window: the icon bundle, the stored choice (read from the
/// preference store), the stylesheets, the portal follower, and the Monet seed read. A seed that
/// arrives later re-renders the palette (T-49a-005, T-49a-058).
pub fn startup() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    icons::register(&display);
    let store = crate::data::app().base.store().clone();
    apply::install(&display, prefs::read_settings(&store));
    apply::follow_system();
    monet::read_seed(|_seed| {
        apply::refresh();
    });
}

/// Tracks the window's size class and records each change (T-49a-058). Call once the window exists.
pub fn attach_window(window: &impl gtk::prelude::IsA<gtk::Window>) {
    size_class::attach(window, |class| {
        WINDOW_CLASS.with(|c| c.set(class));
        tracing::debug!(?class, "window size class changed");
    });
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

/// Entry point: settings and the system dark flag in, tokens out. Reads the stored Monet seed,
/// then calls `resolve_with_seed`; the seed lives in `monet` and is set by the startup read.
pub fn resolve(settings: &ThemeSettings, system_dark: bool) -> Resolved {
    resolve_with_seed(settings, system_dark, monet::seed(), monet::contrast())
}

/// Pure resolution with an explicit Monet seed (no GTK calls, unit-tested).
/// Monet with a seed generates its palette; Monet without one resolves to `System` (E-49-06).
pub fn resolve_with_seed(
    settings: &ThemeSettings,
    system_dark: bool,
    seed: Option<tokens::Rgba>,
    contrast: f64,
) -> Resolved {
    let dark = is_dark(settings.mode, system_dark);
    let monet_choice = settings.scheme == SchemeSel::Scheme(Scheme::MONET);
    if let (true, Some(seed)) = (monet_choice, seed) {
        let mut tokens = monet::generate(seed, dark, contrast);
        if dark && settings.amoled {
            resolve::apply_amoled(&mut tokens.colors);
        }
        tokens.amoled = dark && settings.amoled;
        return Resolved {
            tokens,
            system_scheme: false,
        };
    }
    let app = match settings.scheme {
        SchemeSel::Scheme(app) if !monet_choice => app,
        _ => Scheme::DEFAULT,
    };
    let scheme = resolve::scheme_for(app);
    Resolved {
        tokens: resolve::tokens_for(scheme, dark, settings.amoled),
        system_scheme: settings.scheme == SchemeSel::System || monet_choice,
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

    // T-49a-007: a stored Monet with no seed resolves to System.
    #[test]
    fn monet_without_a_seed_resolves_to_system() {
        let s = ThemeSettings {
            scheme: SchemeSel::Scheme(Scheme::MONET),
            mode: ThemeMode::LIGHT,
            ..ThemeSettings::default()
        };
        let none = resolve_with_seed(&s, false, None, 0.0);
        assert!(none.system_scheme);
        assert_eq!(none.tokens, resolve_with_seed(&ThemeSettings::default(), false, None, 0.0).tokens);
        let seeded = resolve_with_seed(&s, false, Some(Rgba::argb(0xFF1E_88E5)), 0.0);
        assert!(!seeded.system_scheme);
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
