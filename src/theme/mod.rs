//! Theme: colour tokens, scheme resolution, GTK CSS, and the thin widgets built on them.
//!
//! The pure parts (`tokens`, `schemes`, `baseline`, `resolve`, `css`, `dark`, `prefs`, `app_theme`,
//! `style`, `icons`, `empty_state`, `loading`, `cover`) have unit tests and no GTK calls. `apply`,
//! `components` and the widget functions of the other modules are the GTK layer.
//!
//! `startup` is the single call `main.rs` makes before the first window is presented (T-49a-058).
//! The other helpers are used by the screens as their lanes adopt them.
//!
//! Android theme items that Kurobayomi does not port (area 49b, T-49ba-007, 008, 023, 033, 039,
//! 040, 042, 047). Each has one reason here:
//! - Dynamic colour: the Android `m3_sys_color_dynamic_*` tokens are never read and there is no dynamic scheme.
//!   A stored `MONET` reads as the GTK theme (`prefs`).
//! - `lightStatusBar`, `windowLightStatusBar`, `lightNavigationBar`, `elevationOverlayEnabled`, `windowLightNavigationBar`
//!   and the API-27 navigation-bar overrides: GTK has no system bars and no forced dark mode; the
//!   dark decision comes from the window background (`dark::`).
//! - `forceDarkAllowed`, `statusBarColor`, `navigationBarColor`, `navigationBarDividerColor`,
//!   `enforceNavigationBarContrast`, `enforceStatusBarContrast`, `windowActionModeOverlay`,
//!   `elevationOverlayColor`: the same reason, no system bars and no elevation tint.
//! - The More-tab three-dot morph: decorative, and the Android `time_group` target has no children.
//! - `PreferenceThemeOverlay.Tachiyomi` items and `Switch` elevation: GTK has no preference overlay,
//!   and `gtk::Switch` has no elevation tint.
//! - `MinMaxNumberPicker` and `MaterialSpinnerView`: numeric settings use `gtk::SpinButton`.
//! - Widget corner radii (`appwidget_*`): there is no home-screen widget on Linux.
//! - `ActionButton.Overflow` and the unused AppCompat full-screen style: no reference in the app.
#![allow(dead_code)]

pub mod anim;
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
pub mod motion;
pub mod picker;
pub mod prefs;
pub mod resolve;
pub mod shared;
pub mod size_class;
pub mod splash;
mod schemes;
#[cfg(test)]
mod schemes_xml;
pub mod style;
pub mod tokens;
pub mod tokyo_night;
#[cfg(test)]
mod tests_49;

use kuro_prefs::ui::{AppTheme as Scheme, ThemeMode};

pub use app_theme::AppTheme;
pub use tokens::Tokens;

/// The colour scheme the user picked. `System` leaves the GTK theme's own colours in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemeSel {
    /// Linux default for new installs. Emits no GTK named-colour overrides.
    System,
    /// One of the Aniyomi schemes.
    Scheme(Scheme),
    /// The bundled Kurobayomi Tokyo Night scheme (T-49a-063), stored as `TOKYO_NIGHT`.
    TokyoNight,
}

impl From<AppTheme> for SchemeSel {
    /// `GtkTheme` and the deprecated names have no scheme, so they select `System`.
    fn from(theme: AppTheme) -> Self {
        if theme == AppTheme::TokyoNight {
            return SchemeSel::TokyoNight;
        }
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
/// preference store), the stylesheets and the portal follower (T-49a-058).
pub fn startup() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    icons::register(&display);
    let store = crate::data::app().base.store().clone();
    apply::install(&display, prefs::read_settings(&store));
    follow_mode(&kuro_prefs::ui::UiPrefs::new(store).theme_mode);
    apply::follow_system();
}

/// Applies the theme mode again on every change of `pref`, from anywhere (a backup import, a second window), not
/// only while the Appearance page is open (T-01a-021). The apply is a no-op when the mode is already the applied one.
fn follow_mode(pref: &kuro_prefs::pref::Pref<ThemeMode>) {
    crate::pref_model::bind::follow(pref, || true, crate::pref_model::widgets::theme_mode::apply);
}

/// Tracks the window's size class and records each change (T-49a-058). Call once the window exists.
pub fn attach_window(window: &impl gtk::prelude::IsA<gtk::Window>) {
    apply::mark_new_window(window.as_ref());
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

/// Entry point: settings and the system dark flag in, tokens out (pure, no GTK calls, unit-tested).
pub fn resolve(settings: &ThemeSettings, system_dark: bool) -> Resolved {
    let dark = is_dark(settings.mode, system_dark);
    if settings.scheme == SchemeSel::TokyoNight {
        return Resolved {
            tokens: resolve::tokens_for(&tokyo_night::TOKYO_NIGHT, dark, settings.amoled),
            system_scheme: false,
        };
    }
    let app = match settings.scheme {
        SchemeSel::Scheme(app) => app,
        _ => Scheme::DEFAULT,
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

    // T-01a-021: a change of the stored mode from anywhere is applied, with no Appearance page open.
    #[test]
    fn a_stored_mode_change_is_applied_without_the_appearance_page() {
        crate::testing::with_display(|| {
            let Some(display) = gtk::gdk::Display::default() else { return };
            apply::install(&display, ThemeSettings::default());
            let pref = kuro_prefs::ui::UiPrefs::new(kuro_prefs::store::PrefStore::in_memory()).theme_mode;
            follow_mode(&pref);
            let prefers_dark = || gtk::Settings::default().is_some_and(|s| s.is_gtk_application_prefer_dark_theme());
            let pump = |ms: u64| {
                let until = std::time::Instant::now() + std::time::Duration::from_millis(ms);
                while std::time::Instant::now() < until {
                    gtk::glib::MainContext::default().iteration(false);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            };
            pref.set(ThemeMode::DARK).unwrap();
            pump(600);
            assert!(prefers_dark());
            pref.set(ThemeMode::LIGHT).unwrap();
            pump(600);
            assert!(!prefers_dark());
        });
    }
}
