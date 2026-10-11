//! What the App theme strip (`pref_model::widgets::app_theme`) shares with the theme module: the card border
//! sizes, the tokens a card previews, and the write that stores a choice and re-renders the palette at once.
//!
//! Each card renders its own scheme through `resolve`, not the live scheme.

use kuro_prefs::store::PrefStore;
use kuro_prefs::ui::ThemeMode;

use super::app_theme::AppTheme;
use super::apply;
use super::prefs;
use super::tokens::Tokens;
use super::{SchemeSel, ThemeSettings, resolve};

/// Border width in px; primary when the card is selected.
pub const CARD_BORDER: i32 = 4;
/// Outer corner radius in px.
pub const CARD_RADIUS_OUTER: i32 = 17;

/// The tokens a card previews: its own scheme in the chosen mode, not the live one.
pub fn preview_tokens(app: AppTheme, mode: ThemeMode, amoled: bool, system_dark: bool) -> Tokens {
    let settings = ThemeSettings {
        scheme: SchemeSel::from(app),
        amoled,
        mode,
    };
    resolve(&settings, system_dark).tokens
}

/// Writes a change to the stored theme inputs and re-renders the palette at once (no restart).
pub fn change_settings_in(store: &PrefStore, change: impl FnOnce(&mut ThemeSettings)) {
    let mut settings = prefs::read_settings(store);
    change(&mut settings);
    prefs::write_scheme(store, settings.scheme);
    prefs::write_mode(store, settings.mode);
    prefs::write_amoled(store, settings.amoled);
    if let Some(display) = gtk::gdk::Display::default() {
        apply::set_settings(&display, settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::tokens::Role;
    use kuro_prefs::ui::AppTheme as Scheme;

    #[test]
    fn each_card_previews_its_own_scheme() {
        let cloudflare = preview_tokens(AppTheme::Cloudflare, ThemeMode::LIGHT, false, false);
        let doom = preview_tokens(AppTheme::Doom, ThemeMode::LIGHT, false, false);
        assert_ne!(cloudflare.get(Role::Primary), doom.get(Role::Primary));
        assert_eq!(
            cloudflare.get(Role::Primary),
            resolve(
                &ThemeSettings {
                    scheme: SchemeSel::Scheme(Scheme::CLOUDFLARE),
                    amoled: false,
                    mode: ThemeMode::LIGHT,
                },
                false,
            )
            .tokens
            .get(Role::Primary)
        );
    }
}
