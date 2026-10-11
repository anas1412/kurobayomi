//! Theme preferences over the area 02 store: keys, defaults, the Linux default, and migration.
//!
//! `pref_app_theme` is read raw so that the Linux-only value `SYSTEM` (no GTK override) can be
//! stored next to the Aniyomi names. `kuro_prefs::ui::AppTheme` does not know `SYSTEM`.

use kuro_prefs::store::PrefStore;
use kuro_prefs::ui::{AppTheme, ThemeMode};
use kuro_prefs::values::PrefValue;

use super::{SchemeSel, ThemeSettings};

pub const KEY_APP_THEME: &str = "pref_app_theme";
pub const KEY_THEME_MODE: &str = "pref_theme_mode_key";

/// The stored name for the Linux-only `System` choice.
pub const SYSTEM_NAME: &str = "SYSTEM";
/// The stored name for the bundled Tokyo Night scheme (T-49a-063).
pub const TOKYO_NIGHT_NAME: &str = "TOKYO_NIGHT";

/// The Android names that are no longer offered: the three deprecated themes and the dynamic `MONET` (it needs
/// Android's dynamic colours, which a desktop does not have). They are rewritten to the Linux default once, on read.
///
/// Decision (T-47ba-042, T-49ba-002, deliberate deviation from their text): the acceptance says these read as
/// `DEFAULT` and are written back. On Linux the default is the GTK theme (`SYSTEM`, T-49b-006), so they are
/// rewritten to `SYSTEM`, not `DEFAULT`. The tests of this module (`read_settings`) pin the rule; the onboarding
/// list (`app_theme::normalise`) maps them to `DEFAULT` because its list has no GTK theme entry.
const DEPRECATED: [&str; 4] = ["MONET", "DARK_BLUE", "HOT_PINK", "BLUE"];

/// What a raw `pref_app_theme` string means. `None` is the unknown-value case.
fn decode_scheme(raw: &str) -> Option<SchemeSel> {
    if raw == SYSTEM_NAME {
        return Some(SchemeSel::System);
    }
    if raw == TOKYO_NIGHT_NAME {
        return Some(SchemeSel::TokyoNight);
    }
    if DEPRECATED.contains(&raw) {
        return Some(SchemeSel::System);
    }
    AppTheme::from_name(raw).map(SchemeSel::Scheme)
}

/// Reads the theme settings. An unset `pref_app_theme` is the Linux default (`System`) and is
/// written as `SYSTEM` on this first read; an unknown one reads as `System` and is left alone. A
/// deprecated name is rewritten to `SYSTEM` once.
pub fn read_settings(store: &PrefStore) -> ThemeSettings {
    let scheme = match store.get_raw(KEY_APP_THEME) {
        None => {
            // First run: record the Linux default so the stored value matches what is shown.
            write_scheme(store, SchemeSel::System);
            SchemeSel::System
        }
        Some(PrefValue::Str(raw)) => {
            if DEPRECATED.contains(&raw.as_str()) {
                tracing::debug!(value = %raw, "rewriting deprecated app theme to System");
                write_scheme(store, SchemeSel::System);
            }
            match decode_scheme(&raw) {
                Some(sel) => sel,
                None => {
                    tracing::debug!(value = %raw, "unknown app theme, using System");
                    SchemeSel::System
                }
            }
        }
        Some(_) => {
            tracing::debug!("app theme has a non-string type, using System");
            SchemeSel::System
        }
    };

    let mode = match store.get_raw(KEY_THEME_MODE) {
        Some(PrefValue::Str(raw)) => ThemeMode::from_name(&raw).unwrap_or(ThemeMode::SYSTEM),
        _ => ThemeMode::SYSTEM,
    };

    // Pure black is cut (settings diet): the dark mode is the only dark look, so the flag is always off.
    ThemeSettings {
        scheme,
        amoled: false,
        mode,
    }
}

/// Writes the scheme choice (`System` is stored as `SYSTEM`).
pub fn write_scheme(store: &PrefStore, scheme: SchemeSel) {
    let name = match scheme {
        SchemeSel::System => SYSTEM_NAME,
        SchemeSel::TokyoNight => TOKYO_NIGHT_NAME,
        SchemeSel::Scheme(app) => app.name(),
    };
    store.set_raw(KEY_APP_THEME, PrefValue::Str(name.to_string()));
}

/// Writes the mode choice.
pub fn write_mode(store: &PrefStore, mode: ThemeMode) {
    store.set_raw(KEY_THEME_MODE, PrefValue::Str(mode.name().to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(app: Option<&str>) -> PrefStore {
        let store = PrefStore::in_memory();
        if let Some(a) = app {
            store.set_raw(KEY_APP_THEME, PrefValue::Str(a.to_string()));
        }
        store
    }

    #[test]
    fn unset_theme_is_the_linux_default_system() {
        let s = read_settings(&store_with(None));
        assert_eq!(s.scheme, SchemeSel::System);
        assert_eq!(s.mode, ThemeMode::SYSTEM);
        assert!(!s.amoled);
    }

    #[test]
    fn tokyo_night_round_trips_through_the_store() {
        let store = store_with(Some("TOKYO_NIGHT"));
        assert_eq!(read_settings(&store).scheme, SchemeSel::TokyoNight);
        write_scheme(&store, SchemeSel::TokyoNight);
        assert_eq!(store.get_raw(KEY_APP_THEME), Some(PrefValue::Str("TOKYO_NIGHT".into())));
    }

    #[test]
    fn first_read_writes_system_once() {
        let store = store_with(None);
        assert_eq!(store.get_raw(KEY_APP_THEME), None);
        read_settings(&store);
        assert_eq!(store.get_raw(KEY_APP_THEME), Some(PrefValue::Str("SYSTEM".into())));
        // A later choice is not overwritten by another read.
        write_scheme(&store, SchemeSel::Scheme(AppTheme::NORD));
        assert_eq!(read_settings(&store).scheme, SchemeSel::Scheme(AppTheme::NORD));
    }

    #[test]
    fn unknown_name_is_not_overwritten() {
        let store = store_with(Some("FOO"));
        read_settings(&store);
        assert_eq!(store.get_raw(KEY_APP_THEME), Some(PrefValue::Str("FOO".into())));
    }

    #[test]
    fn unknown_name_reads_as_system_without_error() {
        let s = read_settings(&store_with(Some("FOO")));
        assert_eq!(s.scheme, SchemeSel::System);
    }

    #[test]
    fn known_name_round_trips() {
        let store = store_with(None);
        write_scheme(&store, SchemeSel::Scheme(AppTheme::GREEN_APPLE));
        assert_eq!(read_settings(&store).scheme, SchemeSel::Scheme(AppTheme::GREEN_APPLE));
    }

    #[test]
    fn system_round_trips_as_system_name() {
        let store = store_with(None);
        write_scheme(&store, SchemeSel::System);
        assert_eq!(store.get_raw(KEY_APP_THEME), Some(PrefValue::Str("SYSTEM".into())));
        assert_eq!(read_settings(&store).scheme, SchemeSel::System);
    }

    #[test]
    fn deprecated_names_are_rewritten_once_to_system() {
        for old in DEPRECATED {
            let store = store_with(Some(old));
            assert_eq!(read_settings(&store).scheme, SchemeSel::System);
            assert_eq!(store.get_raw(KEY_APP_THEME), Some(PrefValue::Str("SYSTEM".into())));
        }
    }

    #[test]
    fn a_stored_monet_falls_back_to_the_gtk_theme_and_is_written_back() {
        let store = store_with(Some("MONET"));
        assert_eq!(read_settings(&store).scheme, SchemeSel::System);
        assert_eq!(store.get_raw(KEY_APP_THEME), Some(PrefValue::Str("SYSTEM".into())));
    }

    #[test]
    fn mode_reads_its_value_and_pure_black_stays_off() {
        let store = store_with(None);
        write_mode(&store, ThemeMode::DARK);
        store.set_raw("pref_theme_dark_amoled_key", PrefValue::Bool(true));
        let s = read_settings(&store);
        assert_eq!(s.mode, ThemeMode::DARK);
        assert!(!s.amoled);
    }

    // T-49b-006: the Linux default is the GTK theme, stored as SYSTEM; both spec files say the same, so the first
    // value written and the value the spec shows agree.
    #[test]
    fn the_spec_defaults_of_the_app_theme_match_the_first_value_written() {
        use crate::spec::Item;
        assert_eq!(SYSTEM_NAME, "SYSTEM");
        crate::ui::with_spec(|spec| {
            let mut seen = 0;
            for id in ["settings_appearance", "onboarding"] {
                let screen = spec.screen(id).unwrap_or_else(|| panic!("{id} is a spec screen"));
                for item in screen.groups.iter().flat_map(|g| &g.items) {
                    let (title, options, default) = match item {
                        Item::Chips { title, options, default } | Item::List { title, options, default, .. } => (title, options, default),
                        _ => continue,
                    };
                    if title != "App theme" {
                        continue;
                    }
                    seen += 1;
                    assert_eq!(default.as_str(), Some("System"), "{id}");
                    assert_eq!(options.first().map(String::as_str), Some("System"), "{id}: the GTK theme is the first choice");
                }
            }
            assert_eq!(seen, 2, "one App theme item on each screen");
        });
    }
}
