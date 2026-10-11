//! TEST-49-01 to 05 and TEST-49-22 of the area plan (`p1-edge-cases-and-tests.md` section 5), under the ids the plan
//! gives them. The other cases are tagged in the module they test.

use kuro_prefs::store::PrefStore;
use kuro_prefs::ui::AppTheme as Scheme;
use kuro_prefs::values::PrefValue;

use super::resolve::tokens_for;
use super::schemes::SCHEMES;
use super::shared::AMOLED_BACKGROUND;
use super::tokens::Role;
use super::{prefs, SchemeSel};

/// The 21 stored names of Aniyomi's `AppTheme` this app keeps, in declaration order (`p1-data-and-config.md` 1.1).
/// `MONET` (Android dynamic colour) is not one of them: it reads as the GTK theme.
const ANDROID_NAMES: [&str; 21] = [
    "DEFAULT", "CLOUDFLARE", "COTTONCANDY", "DOOM", "GREEN_APPLE", "LAVENDER", "MATRIX", "MIDNIGHT_DUSK", "MOCHA",
    "SAPPHIRE", "NORD", "STRAWBERRY_DAIQUIRI", "TAKO", "TEALTURQUOISE", "TIDAL_WAVE", "YINYANG", "YOTSUBA", "MONOCHROME",
    "DARK_BLUE", "HOT_PINK", "BLUE",
];

/// The title key of each name, `None` for the three deprecated ones (`p1-behavior.md` 12).
const TITLE_KEYS: [Option<&str>; 21] = [
    Some("label_default"),
    Some("theme_cloudflare"),
    Some("theme_cottoncandy"),
    Some("theme_doom"),
    Some("theme_greenapple"),
    Some("theme_lavender"),
    Some("theme_matrix"),
    Some("theme_midnightdusk"),
    Some("theme_mocha"),
    Some("theme_sapphire"),
    Some("theme_nord"),
    Some("theme_strawberrydaiquiri"),
    Some("theme_tako"),
    Some("theme_tealturquoise"),
    Some("theme_tidalwave"),
    Some("theme_yinyang"),
    Some("theme_yotsuba"),
    Some("theme_monochrome"),
    None,
    None,
    None,
];

fn stored(raw: &str) -> PrefStore {
    let store = PrefStore::in_memory();
    store.set_raw(prefs::KEY_APP_THEME, PrefValue::Str(raw.to_owned()));
    store
}

#[test]
fn test_49_01_all_21_android_names_round_trip_and_the_retired_four_read_as_the_default() {
    for name in ANDROID_NAMES {
        let theme = Scheme::from_name(name).unwrap_or_else(|| panic!("{name} parses"));
        assert_eq!(theme.name(), name);
    }
    for name in ["MONET", "DARK_BLUE", "HOT_PINK", "BLUE"] {
        let store = stored(name);
        assert_eq!(prefs::read_settings(&store).scheme, SchemeSel::System, "{name} reads as the Linux default");
        assert_eq!(store.get_raw(prefs::KEY_APP_THEME), Some(PrefValue::Str("SYSTEM".to_owned())), "{name} is rewritten once");
    }
    for name in ["mocha", "", "NOT_A_THEME"] {
        let store = stored(name);
        assert_eq!(prefs::read_settings(&store).scheme, SchemeSel::System, "{name:?} reads as the default");
        assert_eq!(store.get_raw(prefs::KEY_APP_THEME), Some(PrefValue::Str(name.to_owned())), "{name:?} is not written back");
    }
}

#[test]
fn test_49_02_every_theme_has_the_title_key_of_the_plan() {
    for (name, key) in ANDROID_NAMES.into_iter().zip(TITLE_KEYS) {
        assert_eq!(Scheme::from_name(name).expect(name).title_key(), key, "{name}");
    }
}

#[test]
fn test_49_03_dark_resolution_equals_the_scheme_dark_table_for_all_18_schemes() {
    for scheme in SCHEMES.iter() {
        let tokens = tokens_for(scheme, true, false);
        assert!(tokens.dark && !tokens.amoled, "{}", scheme.name);
        for role in Role::ALL {
            if let Some(explicit) = scheme.dark[role as usize] {
                assert_eq!(tokens.get(role), explicit, "{} {role:?}", scheme.name);
            }
        }
    }
}

/// The ten roles `BaseColorScheme` overrides for AMOLED.
const AMOLED_ROLES: [Role; 10] = [
    Role::Background,
    Role::OnBackground,
    Role::Surface,
    Role::OnSurface,
    Role::SurfaceVariant,
    Role::SurfaceContainerLowest,
    Role::SurfaceContainerLow,
    Role::SurfaceContainer,
    Role::SurfaceContainerHigh,
    Role::SurfaceContainerHighest,
];

#[test]
fn test_49_04_dark_amoled_changes_exactly_the_ten_overrides() {
    for scheme in SCHEMES.iter() {
        let (plain, black) = (tokens_for(scheme, true, false), tokens_for(scheme, true, true));
        assert!(black.amoled, "{}", scheme.name);
        assert_eq!(black.get(Role::Background), AMOLED_BACKGROUND, "{}", scheme.name);
        for role in Role::ALL {
            if !AMOLED_ROLES.contains(&role) {
                assert_eq!(black.get(role), plain.get(role), "{} {role:?} is not an AMOLED override", scheme.name);
            }
        }
    }
}

#[test]
fn test_49_05_light_resolution_ignores_amoled() {
    for scheme in SCHEMES.iter() {
        let (plain, black) = (tokens_for(scheme, false, false), tokens_for(scheme, false, true));
        assert_eq!(plain.colors, black.colors, "{}", scheme.name);
        assert!(!black.amoled);
    }
}

#[test]
fn test_49_22_slider_steps_default_to_last_minus_first_minus_one_and_never_go_negative() {
    use crate::pref_model::model::slider_steps;
    assert_eq!(slider_steps(0, 10), 9);
    assert_eq!(slider_steps(1, 2), 0);
    assert_eq!(slider_steps(5, 5), 0);
    assert_eq!(slider_steps(5, 3), 0, "a reversed range is not negative");
}
