//! The bundled Tokyo Night scheme (T-49a-063). A Kurobayomi addition, not an Aniyomi `AppTheme`:
//! it has no Android name, and it is stored as `TOKYO_NIGHT`.
//!
//! The roles are mapped by hand from the Tokyo Night palette (night for dark, day for light). The
//! light table is the Day variant. Each table row names its role; the order is `Role` index order.
//!
//! Palette sources (hex): bg `1a1b26`, bg_dark `16161e`, bg_highlight `292e42`, fg `c0caf5`,
//! fg_dark `a9b1d6`, dark3 `545c7e`, comment `565f89`, dark5 `737aa2`, blue `7aa2f7`, blue0
//! `3d59a1`, cyan `7dcfff`, magenta `bb9af7`, green `9ece6a`, green1 `73daca`, red `f7768e`; Day:
//! bg `e1e2e7`, fg `3760bf`, blue `2e7de9`, purple `9854f1`, green `587539`, red `f52a65`, comment
//! `848cb5`, bg_highlight `c4c8da`.

use super::schemes::SchemeDef;
use super::tokens::Rgba;

/// `Some` of an opaque or translucent ARGB literal, for the const tables below.
const fn o(v: u32) -> Option<Rgba> {
    Some(Rgba::argb(v))
}

/// The scheme itself. Monet and System do not use it; `SchemeSel::TokyoNight` resolves to it.
pub const TOKYO_NIGHT: SchemeDef = SchemeDef {
    name: "TokyoNight",
    light: [
        o(0xFF2E_7DE9), // primary
        o(0xFFFF_FFFF), // onPrimary
        o(0xFFD5_E2FF), // primaryContainer
        o(0xFF0D_3D8C), // onPrimaryContainer
        o(0xFF7A_A2F7), // inversePrimary
        o(0xFF98_54F1), // secondary
        o(0xFFFF_FFFF), // onSecondary
        o(0xFFEB_DCFF), // secondaryContainer
        o(0xFF3D_1A7A), // onSecondaryContainer
        o(0xFF58_7539), // tertiary
        o(0xFFFF_FFFF), // onTertiary
        o(0xFFD7_EABB), // tertiaryContainer
        o(0xFF24_3A12), // onTertiaryContainer
        o(0xFFE1_E2E7), // background
        o(0xFF37_60BF), // onBackground
        o(0xFFE1_E2E7), // surface
        o(0xFF37_60BF), // onSurface
        o(0xFFC4_C8DA), // surfaceVariant
        o(0xFF61_72B0), // onSurfaceVariant
        o(0xFF2E_7DE9), // surfaceTint
        o(0xFF37_60BF), // inverseSurface
        o(0xFFE1_E2E7), // inverseOnSurface
        o(0xFFF5_2A65), // error
        o(0xFFFF_FFFF), // onError
        o(0xFFFF_D9E0), // errorContainer
        o(0xFF7A_0E2B), // onErrorContainer
        o(0xFF89_90B3), // outline
        o(0xFFB9_BFD6), // outlineVariant
        o(0xFF00_0000), // scrim
        o(0xFFE9_EAEE), // surfaceBright
        o(0xFFD5_D6DB), // surfaceDim
        o(0xFFD9_DAE0), // surfaceContainer
        o(0xFFD0_D2DA), // surfaceContainerHigh
        o(0xFFC4_C8DA), // surfaceContainerHighest
        o(0xFFDF_E0E6), // surfaceContainerLow
        o(0xFFEB_ECF0), // surfaceContainerLowest
        None,           // primaryInverse (alias; inversePrimary is set)
        None,           // elevationOverlay (resolves to primary)
    ],
    dark: [
        o(0xFF7A_A2F7), // primary
        o(0xFF1A_1B26), // onPrimary
        o(0xFF3D_59A1), // primaryContainer
        o(0xFFDB_E4FF), // onPrimaryContainer
        o(0xFF3D_59A1), // inversePrimary
        o(0xFFBB_9AF7), // secondary
        o(0xFF1A_1B26), // onSecondary
        o(0xFF4A_3D6B), // secondaryContainer
        o(0xFFE7_DCFF), // onSecondaryContainer
        o(0xFF73_DACA), // tertiary
        o(0xFF1A_1B26), // onTertiary
        o(0xFF2F_4D46), // tertiaryContainer
        o(0xFFB4_F9F8), // onTertiaryContainer
        o(0xFF1A_1B26), // background
        o(0xFFC0_CAF5), // onBackground
        o(0xFF1A_1B26), // surface
        o(0xFFC0_CAF5), // onSurface
        o(0xFF29_2E42), // surfaceVariant
        o(0xFFA9_B1D6), // onSurfaceVariant
        o(0xFF7A_A2F7), // surfaceTint
        o(0xFFC0_CAF5), // inverseSurface
        o(0xFF1A_1B26), // inverseOnSurface
        o(0xFFF7_768E), // error
        o(0xFF1A_1B26), // onError
        o(0xFF6B_2C3C), // errorContainer
        o(0xFFFF_D9DE), // onErrorContainer
        o(0xFF73_7AA2), // outline
        o(0xFF3B_4261), // outlineVariant
        o(0xFF00_0000), // scrim
        o(0xFF34_394F), // surfaceBright
        o(0xFF16_161E), // surfaceDim
        o(0xFF1F_2335), // surfaceContainer
        o(0xFF24_283B), // surfaceContainerHigh
        o(0xFF2F_3549), // surfaceContainerHighest
        o(0xFF1B_1D2B), // surfaceContainerLow
        o(0xFF16_161E), // surfaceContainerLowest
        None,           // primaryInverse (alias; inversePrimary is set)
        None,           // elevationOverlay (resolves to primary)
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::resolve::tokens_for;
    use crate::theme::tokens::Role;
    use crate::theme::{resolve_with_seed, SchemeSel, ThemeSettings};
    use kuro_prefs::ui::ThemeMode;

    #[test]
    fn documented_primaries_for_each_mode() {
        assert_eq!(TOKYO_NIGHT.dark[Role::Primary as usize], Some(Rgba::argb(0xFF7A_A2F7)));
        assert_eq!(TOKYO_NIGHT.light[Role::Primary as usize], Some(Rgba::argb(0xFF2E_7DE9)));
    }

    #[test]
    fn dark_background_is_the_night_background() {
        let t = tokens_for(&TOKYO_NIGHT, true, false);
        assert_eq!(t.get(Role::Background), Rgba::parse_hex("#1a1b26").unwrap());
        assert!(t.dark);
    }

    #[test]
    fn css_passes_the_offline_check_in_both_modes_and_amoled() {
        for dark in [false, true] {
            for amoled in [false, true] {
                let css = crate::theme::css::render(&tokens_for(&TOKYO_NIGHT, dark, amoled), false);
                assert_eq!(crate::theme::css::check(&css), Ok(()), "dark={dark} amoled={amoled}");
            }
        }
    }

    #[test]
    fn selecting_tokyo_night_resolves_to_its_palette_and_overrides_named_colours() {
        let settings = ThemeSettings {
            scheme: SchemeSel::TokyoNight,
            amoled: false,
            mode: ThemeMode::DARK,
        };
        let resolved = resolve_with_seed(&settings, false, None, 0.0);
        assert!(!resolved.system_scheme);
        assert_eq!(resolved.tokens, tokens_for(&TOKYO_NIGHT, true, false));
    }
}
