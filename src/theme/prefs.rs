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
pub const KEY_AMOLED: &str = "pref_theme_dark_amoled_key";

/// The stored name for the Linux-only `System` choice.
pub const SYSTEM_NAME: &str = "SYSTEM";

/// The deprecated Android names. They are rewritten to the Linux default once, on read.
const DEPRECATED: [&str; 3] = ["DARK_BLUE", "HOT_PINK", "BLUE"];

/// What a raw `pref_app_theme` string means. `None` is the unknown-value case.
fn decode_scheme(raw: &str) -> Option<SchemeSel> {
    if raw == SYSTEM_NAME {
        return Some(SchemeSel::System);
    }
    if DEPRECATED.contains(&raw) {
        return Some(SchemeSel::System);
    }
    let app = AppTheme::from_name(raw)?;
    Some(match app {
        // Monet needs a seed and HCT generation (not built yet), so it is unavailable and shows
        // the System card, as the design requires for an unavailable Monet.
        AppTheme::MONET => SchemeSel::System,
        other => SchemeSel::Scheme(other),
    })
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

    let amoled = match store.get_raw(KEY_AMOLED) {
        Some(PrefValue::Bool(b)) => b,
        _ => false,
    };

    ThemeSettings {
        scheme,
        amoled,
        mode,
    }
}

/// Writes the scheme choice (`System` is stored as `SYSTEM`).
pub fn write_scheme(store: &PrefStore, scheme: SchemeSel) {
    let name = match scheme {
        SchemeSel::System => SYSTEM_NAME,
        SchemeSel::Scheme(app) => app.name(),
    };
    store.set_raw(KEY_APP_THEME, PrefValue::Str(name.to_string()));
}

/// Writes the mode choice.
pub fn write_mode(store: &PrefStore, mode: ThemeMode) {
    store.set_raw(KEY_THEME_MODE, PrefValue::Str(mode.name().to_string()));
}

/// Writes the AMOLED flag.
pub fn write_amoled(store: &PrefStore, amoled: bool) {
    store.set_raw(KEY_AMOLED, PrefValue::Bool(amoled));
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
    fn stored_monet_resolves_to_system_while_unavailable() {
        assert_eq!(read_settings(&store_with(Some("MONET"))).scheme, SchemeSel::System);
    }

    #[test]
    fn mode_and_amoled_read_their_values() {
        let store = store_with(None);
        write_mode(&store, ThemeMode::DARK);
        write_amoled(&store, true);
        let s = read_settings(&store);
        assert_eq!(s.mode, ThemeMode::DARK);
        assert!(s.amoled);
    }
}
