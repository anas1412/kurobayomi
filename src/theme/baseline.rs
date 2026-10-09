//! Material 3 baseline colours, used for every role a scheme leaves out (T-49a-013, T-49b-004).
//!
//! Compose fills omitted roles from `darkColorScheme()` / `lightColorScheme()`. Replicate that and
//! do not improve it: a scheme that omits `error` shows the baseline error colours, as Aniyomi does.
//!
//! The values below are `androidx.compose.material3` **1.4.0** `ColorDarkTokens` / `ColorLightTokens`
//! (the version the Compose BOM `2025.10.01` pinned in `gradle/compose.versions.toml` resolves to),
//! dumped from the `material3-android-1.4.0.aar` `classes.jar` by reading the token class
//! initialisers (`javap -c`) and resolving each `PaletteTokens` getter to its RGB constant.
//! `surfaceTint` is `primary`, as `darkColorScheme()` / `lightColorScheme()` default it.
//! Re-run the dump when the BOM is bumped; `tests::baseline_matches_material3_1_4_0` pins a sample.

use super::tokens::{Role, Rgba};

/// The 36 M3 baseline roles, in `Role` index order (`Role::ALL[0..36]`).
const BASELINE_DARK_ARGB: [u32; 36] = [
    0xFFD0BCFF, // primary
    0xFF381E72, // onPrimary
    0xFF4F378B, // primaryContainer
    0xFFEADDFF, // onPrimaryContainer
    0xFF6750A4, // inversePrimary
    0xFFCCC2DC, // secondary
    0xFF332D41, // onSecondary
    0xFF4A4458, // secondaryContainer
    0xFFE8DEF8, // onSecondaryContainer
    0xFFEFB8C8, // tertiary
    0xFF492532, // onTertiary
    0xFF633B48, // tertiaryContainer
    0xFFFFD8E4, // onTertiaryContainer
    0xFF141218, // background
    0xFFE6E0E9, // onBackground
    0xFF141218, // surface
    0xFFE6E0E9, // onSurface
    0xFF49454F, // surfaceVariant
    0xFFCAC4D0, // onSurfaceVariant
    0xFFD0BCFF, // surfaceTint
    0xFFE6E0E9, // inverseSurface
    0xFF322F35, // inverseOnSurface
    0xFFF2B8B5, // error
    0xFF601410, // onError
    0xFF8C1D18, // errorContainer
    0xFFF9DEDC, // onErrorContainer
    0xFF938F99, // outline
    0xFF49454F, // outlineVariant
    0xFF000000, // scrim
    0xFF3B383E, // surfaceBright
    0xFF141218, // surfaceDim
    0xFF211F26, // surfaceContainer
    0xFF2B2930, // surfaceContainerHigh
    0xFF36343B, // surfaceContainerHighest
    0xFF1D1B20, // surfaceContainerLow
    0xFF0F0D13, // surfaceContainerLowest
];

const BASELINE_LIGHT_ARGB: [u32; 36] = [
    0xFF6750A4, // primary
    0xFFFFFFFF, // onPrimary
    0xFFEADDFF, // primaryContainer
    0xFF21005D, // onPrimaryContainer
    0xFFD0BCFF, // inversePrimary
    0xFF625B71, // secondary
    0xFFFFFFFF, // onSecondary
    0xFFE8DEF8, // secondaryContainer
    0xFF1D192B, // onSecondaryContainer
    0xFF7D5260, // tertiary
    0xFFFFFFFF, // onTertiary
    0xFFFFD8E4, // tertiaryContainer
    0xFF31111D, // onTertiaryContainer
    0xFFFEF7FF, // background
    0xFF1D1B20, // onBackground
    0xFFFEF7FF, // surface
    0xFF1D1B20, // onSurface
    0xFFE7E0EC, // surfaceVariant
    0xFF49454F, // onSurfaceVariant
    0xFF6750A4, // surfaceTint
    0xFF322F35, // inverseSurface
    0xFFF5EFF7, // inverseOnSurface
    0xFFB3261E, // error
    0xFFFFFFFF, // onError
    0xFFF9DEDC, // errorContainer
    0xFF410E0B, // onErrorContainer
    0xFF79747E, // outline
    0xFFCAC4D0, // outlineVariant
    0xFF000000, // scrim
    0xFFFEF7FF, // surfaceBright
    0xFFDED8E1, // surfaceDim
    0xFFF3EDF7, // surfaceContainer
    0xFFECE6F0, // surfaceContainerHigh
    0xFFE6E0E9, // surfaceContainerHighest
    0xFFF7F2FA, // surfaceContainerLow
    0xFFFFFFFF, // surfaceContainerLowest
];

/// The baseline colour for `role` in the given mode.
///
/// The two XML-only aliases have no M3 counterpart in the table: `PrimaryInverseAlias` takes the
/// baseline `InversePrimary`, and `ElevationOverlay` takes the baseline `Primary`.
pub fn fallback_for(role: Role, dark: bool) -> Rgba {
    let table = if dark {
        &BASELINE_DARK_ARGB
    } else {
        &BASELINE_LIGHT_ARGB
    };
    let index = match role {
        Role::PrimaryInverseAlias => Role::InversePrimary as usize,
        Role::ElevationOverlay => Role::Primary as usize,
        other => other as usize,
    };
    Rgba::argb(table[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_matches_material3_1_4_0() {
        // Spot values from the dump; 1.4.0 differs from the older 1.0 palette on these roles.
        assert_eq!(fallback_for(Role::Surface, true), Rgba::argb(0xFF14_1218));
        assert_eq!(fallback_for(Role::OnSurface, true), Rgba::argb(0xFFE6_E0E9));
        assert_eq!(fallback_for(Role::InverseOnSurface, true), Rgba::argb(0xFF32_2F35));
        assert_eq!(fallback_for(Role::Surface, false), Rgba::argb(0xFFFE_F7FF));
        assert_eq!(fallback_for(Role::OnSurface, false), Rgba::argb(0xFF1D_1B20));
        assert_eq!(fallback_for(Role::SurfaceBright, false), Rgba::argb(0xFFFE_F7FF));
        assert_eq!(fallback_for(Role::SurfaceTint, true), fallback_for(Role::Primary, true));
        assert_eq!(fallback_for(Role::SurfaceTint, false), fallback_for(Role::Primary, false));
    }

    #[test]
    fn error_baseline_matches_m3() {
        assert_eq!(fallback_for(Role::Error, true), Rgba::argb(0xFFF2_B8B5));
        assert_eq!(fallback_for(Role::Error, false), Rgba::argb(0xFFB3_261E));
    }

    #[test]
    fn aliases_take_their_counterpart() {
        assert_eq!(
            fallback_for(Role::PrimaryInverseAlias, false),
            fallback_for(Role::InversePrimary, false)
        );
        assert_eq!(
            fallback_for(Role::ElevationOverlay, true),
            fallback_for(Role::Primary, true)
        );
    }

    #[test]
    fn scrim_is_opaque_black_in_both_modes() {
        assert_eq!(fallback_for(Role::Scrim, true), Rgba::argb(0xFF00_0000));
        assert_eq!(fallback_for(Role::Scrim, false), Rgba::argb(0xFF00_0000));
    }
}
