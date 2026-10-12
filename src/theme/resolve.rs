//! Pure scheme resolution (no GTK): the default scheme and the omitted-role fallback.
//!
//! Follows `p2-rust-design.md` section 3.2 (Compose parity, `BaseColorScheme.kt` and
//! `TachiyomiTheme.kt`).

use super::baseline::fallback_for;
use super::schemes::{SchemeDef, DEFAULT_SCHEME};
use super::tokens::{Role, Rgba, Tokens, ROLE_COUNT};

/// Resolves the default scheme for a mode.
pub fn tokens_for(scheme: &SchemeDef, dark: bool) -> Tokens {
    let explicit: [Option<Rgba>; ROLE_COUNT] = if dark { scheme.dark } else { scheme.light };
    let mut table = explicit;

    // Alias: `primaryInverse` stands in for `inversePrimary` when the latter is absent.
    if table[Role::InversePrimary as usize].is_none() {
        table[Role::InversePrimary as usize] = table[Role::PrimaryInverseAlias as usize];
    }

    let mut colors = [Rgba::argb(0); ROLE_COUNT];
    for role in Role::ALL {
        let i = role as usize;
        colors[i] = table[i].unwrap_or_else(|| fallback_for(role, dark));
    }

    // Omitted `surfaceTint` takes the scheme primary (M3 `ColorScheme` default), not the baseline.
    if table[Role::SurfaceTint as usize].is_none() {
        colors[Role::SurfaceTint as usize] = colors[Role::Primary as usize];
    }
    // `elevationOverlay` is the resolved primary when the XML does not give it.
    if table[Role::ElevationOverlay as usize].is_none() {
        colors[Role::ElevationOverlay as usize] = colors[Role::Primary as usize];
    }

    Tokens { colors, dark }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_values_are_kept_in_dark_mode() {
        let t = tokens_for(&DEFAULT_SCHEME, true);
        for role in Role::ALL {
            if let Some(v) = DEFAULT_SCHEME.dark[role as usize] {
                assert_eq!(t.get(role), v, "{:?}", role);
            }
        }
    }

    #[test]
    fn omitted_scrim_is_baseline() {
        // Default's XML has no scrim.
        let t = tokens_for(&DEFAULT_SCHEME, false);
        assert_eq!(t.get(Role::Scrim), fallback_for(Role::Scrim, false));
    }

    #[test]
    fn omitted_surface_tint_is_the_scheme_primary() {
        // The default scheme gives surfaceTint in both modes, so the rule is checked on a scheme that omits it.
        let mut scheme = SchemeDef { name: "test", light: [None; ROLE_COUNT], dark: [None; ROLE_COUNT] };
        scheme.light[Role::Primary as usize] = Some(Rgba::argb(0xFF00_FF00));
        scheme.dark[Role::Primary as usize] = Some(Rgba::argb(0xFF00_00FF));
        for dark in [false, true] {
            let t = tokens_for(&scheme, dark);
            assert_eq!(t.get(Role::SurfaceTint), t.get(Role::Primary), "dark: {dark}");
        }
        // And wherever the default scheme does omit it, the same rule holds.
        for dark in [false, true] {
            let table = if dark { &DEFAULT_SCHEME.dark } else { &DEFAULT_SCHEME.light };
            if table[Role::SurfaceTint as usize].is_none() {
                let t = tokens_for(&DEFAULT_SCHEME, dark);
                assert_eq!(t.get(Role::SurfaceTint), t.get(Role::Primary));
            }
        }
    }

    #[test]
    fn inverse_primary_uses_alias_when_absent() {
        let mut scheme = SchemeDef {
            name: "test",
            light: [None; ROLE_COUNT],
            dark: [None; ROLE_COUNT],
        };
        scheme.light[Role::PrimaryInverseAlias as usize] = Some(Rgba::argb(0xFF12_3456));
        let t = tokens_for(&scheme, false);
        assert_eq!(t.get(Role::InversePrimary), Rgba::argb(0xFF12_3456));
    }

    #[test]
    fn elevation_overlay_defaults_to_primary() {
        let mut scheme = SchemeDef {
            name: "test",
            light: [None; ROLE_COUNT],
            dark: [None; ROLE_COUNT],
        };
        scheme.light[Role::Primary as usize] = Some(Rgba::argb(0xFF00_FF00));
        let t = tokens_for(&scheme, false);
        assert_eq!(t.get(Role::ElevationOverlay), Rgba::argb(0xFF00_FF00));
    }

    /// WCAG relative luminance of an opaque colour.
    fn luminance(c: Rgba) -> f64 {
        let lin = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    }

    fn contrast(fg: Rgba, bg: Rgba) -> f64 {
        // translucent text is composited over its background before measuring
        let (a, b) = (luminance(fg.over(bg)), luminance(bg));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// 4.3: the contrast report. Pairs below WCAG AA (4.5:1) are printed as warnings for the owner and
    /// never fail the test; the test only checks that every pair of both modes was measured.
    #[test]
    fn contrast_report_lists_pairs_below_aa_without_failing() {
        const PAIRS: [(Role, Role); 6] = [
            (Role::OnPrimary, Role::Primary),
            (Role::OnSurface, Role::Surface),
            (Role::OnSurfaceVariant, Role::Surface),
            (Role::OnSecondaryContainer, Role::SecondaryContainer),
            (Role::OnTertiary, Role::Tertiary),
            (Role::OnBackground, Role::Background),
        ];
        let (mut measured, mut warnings) = (0, Vec::new());
        for dark in [false, true] {
            let tokens = tokens_for(&DEFAULT_SCHEME, dark);
            for (fg, bg) in PAIRS {
                let ratio = contrast(tokens.get(fg), tokens.get(bg));
                measured += 1;
                assert!(ratio.is_finite() && ratio >= 1.0, "{fg:?}/{bg:?}");
                if ratio < 4.5 {
                    warnings.push(format!(
                        "{} {fg:?} on {bg:?}: {ratio:.2}:1",
                        if dark { "dark" } else { "light" }
                    ));
                }
            }
        }
        assert_eq!(measured, 2 * PAIRS.len());
        for warning in &warnings {
            eprintln!("contrast warning (below 4.5:1): {warning}");
        }
        eprintln!("contrast report: {} of {measured} pairs below AA", warnings.len());
    }
}
