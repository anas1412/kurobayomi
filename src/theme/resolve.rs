//! Pure scheme resolution (no GTK): scheme lookup, omitted-role fallback, aliases and AMOLED.
//!
//! Follows `p2-rust-design.md` section 3.2 (Compose parity, `BaseColorScheme.kt` and
//! `TachiyomiTheme.kt`).

use kuro_prefs::ui::AppTheme;

use super::baseline::fallback_for;
use super::schemes::{SchemeDef, SCHEMES};
use super::tokens::{Role, Rgba, Tokens, ROLE_COUNT};

/// The `SCHEMES` name for an app theme. The names with no static table (the deprecated ones and the dynamic
/// `MONET`) never reach here and map to the Default scheme.
fn scheme_name(app: AppTheme) -> &'static str {
    match app {
        AppTheme::DEFAULT => "Default",
        AppTheme::CLOUDFLARE => "Cloudflare",
        AppTheme::COTTONCANDY => "CottonCandy",
        AppTheme::DOOM => "Doom",
        AppTheme::GREEN_APPLE => "GreenApple",
        AppTheme::LAVENDER => "Lavender",
        AppTheme::MATRIX => "Matrix",
        AppTheme::MIDNIGHT_DUSK => "MidnightDusk",
        AppTheme::MOCHA => "Mocha",
        AppTheme::SAPPHIRE => "Sapphire",
        AppTheme::NORD => "Nord",
        AppTheme::STRAWBERRY_DAIQUIRI => "StrawberryDaiquiri",
        AppTheme::TAKO => "Tako",
        AppTheme::TEALTURQUOISE => "TealTurquoise",
        AppTheme::TIDAL_WAVE => "TidalWave",
        AppTheme::YINYANG => "YinYang",
        AppTheme::YOTSUBA => "Yotsuba",
        AppTheme::MONOCHROME => "Monochrome",
        _ => "Default",
    }
}

/// The static scheme for an app theme. Unknown or unmapped values fall back to Default.
pub fn scheme_for(app: AppTheme) -> &'static SchemeDef {
    let name = scheme_name(app);
    SCHEMES.iter().find(|s| s.name == name).unwrap_or(&SCHEMES[0])
}

/// Resolves one scheme for a mode. `amoled` applies only in dark mode.
pub fn tokens_for(scheme: &SchemeDef, dark: bool, amoled: bool) -> Tokens {
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

    let amoled = dark && amoled;
    if amoled {
        apply_amoled(&mut colors);
    }
    Tokens {
        colors,
        amoled,
        dark,
    }
}

/// The 10 AMOLED overrides of `BaseColorScheme.getColorScheme`. Nothing else changes.
pub fn apply_amoled(colors: &mut [Rgba; ROLE_COUNT]) {
    use super::shared::*;
    let container = AMOLED_SURFACE_CONTAINER;
    colors[Role::Background as usize] = AMOLED_BACKGROUND;
    colors[Role::OnBackground as usize] = AMOLED_ON_BACKGROUND;
    colors[Role::Surface as usize] = AMOLED_SURFACE;
    colors[Role::OnSurface as usize] = AMOLED_ON_SURFACE;
    colors[Role::SurfaceVariant as usize] = container;
    colors[Role::SurfaceContainerLowest as usize] = container;
    colors[Role::SurfaceContainerLow as usize] = container;
    colors[Role::SurfaceContainer as usize] = container;
    colors[Role::SurfaceContainerHigh as usize] = AMOLED_SURFACE_CONTAINER_HIGH;
    colors[Role::SurfaceContainerHighest as usize] = AMOLED_SURFACE_CONTAINER_HIGHEST;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_values_are_kept_in_dark_mode() {
        for scheme in SCHEMES.iter() {
            let t = tokens_for(scheme, true, false);
            for role in Role::ALL {
                if let Some(v) = scheme.dark[role as usize] {
                    assert_eq!(t.get(role), v, "{} {:?}", scheme.name, role);
                }
            }
        }
    }

    #[test]
    fn light_mode_ignores_amoled() {
        let scheme = scheme_for(AppTheme::DEFAULT);
        assert_eq!(tokens_for(scheme, false, true), tokens_for(scheme, false, false));
        assert!(!tokens_for(scheme, false, true).amoled);
    }

    #[test]
    fn dark_amoled_changes_exactly_the_ten_overrides() {
        let scheme = scheme_for(AppTheme::NORD);
        let plain = tokens_for(scheme, true, false);
        let amoled = tokens_for(scheme, true, true);
        let changed: Vec<Role> = Role::ALL
            .iter()
            .copied()
            .filter(|r| plain.get(*r) != amoled.get(*r))
            .collect();
        assert_eq!(changed.len(), 10, "changed roles: {changed:?}");
        for r in [
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
        ] {
            assert!(changed.contains(&r), "{r:?} not overridden");
        }
        assert!(amoled.amoled);
    }

    #[test]
    fn omitted_roles_take_the_m3_baseline() {
        // Cloudflare has no error roles in the XML.
        let scheme = scheme_for(AppTheme::CLOUDFLARE);
        assert_eq!(
            tokens_for(scheme, true, false).get(Role::Error),
            fallback_for(Role::Error, true)
        );
        assert_eq!(
            tokens_for(scheme, false, false).get(Role::Error),
            fallback_for(Role::Error, false)
        );
    }

    /// TEST-49-07: the named omitted roles equal the literal M3 1.4.0 baseline values.
    #[test]
    fn cloudflare_omitted_roles_equal_the_baseline_literals() {
        let t = tokens_for(scheme_for(AppTheme::CLOUDFLARE), true, false);
        assert_eq!(t.get(Role::OutlineVariant), Rgba::argb(0xFF49_454F));
        assert_eq!(t.get(Role::Scrim), Rgba::argb(0xFF00_0000));
        assert_eq!(t.get(Role::SurfaceDim), Rgba::argb(0xFF14_1218));
        assert_eq!(t.get(Role::SurfaceContainerHigh), Rgba::argb(0xFF2B_2930));
        for role in [Role::OutlineVariant, Role::Scrim, Role::SurfaceDim, Role::SurfaceContainerHigh] {
            assert_eq!(t.get(role), fallback_for(role, true), "{role:?}");
        }
    }

    /// TEST-49-07: GreenApple omits surfaceTint, which then equals its own primary.
    #[test]
    fn green_apple_surface_tint_is_its_primary() {
        for dark in [false, true] {
            let scheme = scheme_for(AppTheme::GREEN_APPLE);
            let t = tokens_for(scheme, dark, false);
            assert_eq!(t.get(Role::SurfaceTint), t.get(Role::Primary), "dark={dark}");
            assert_ne!(t.get(Role::SurfaceTint), fallback_for(Role::SurfaceTint, dark));
        }
    }

    #[test]
    fn omitted_scrim_is_baseline() {
        // Default's XML has no scrim.
        let t = tokens_for(scheme_for(AppTheme::DEFAULT), false, false);
        assert_eq!(t.get(Role::Scrim), fallback_for(Role::Scrim, false));
    }

    #[test]
    fn omitted_surface_tint_is_the_scheme_primary() {
        let mut checked = 0;
        for scheme in SCHEMES.iter() {
            for dark in [false, true] {
                let table = if dark { &scheme.dark } else { &scheme.light };
                if table[Role::SurfaceTint as usize].is_none() {
                    let t = tokens_for(scheme, dark, false);
                    assert_eq!(t.get(Role::SurfaceTint), t.get(Role::Primary), "{}", scheme.name);
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "no scheme omits surfaceTint; the rule is untested");
    }

    #[test]
    fn inverse_primary_uses_alias_when_absent() {
        let mut scheme = SchemeDef {
            name: "test",
            light: [None; ROLE_COUNT],
            dark: [None; ROLE_COUNT],
        };
        scheme.light[Role::PrimaryInverseAlias as usize] = Some(Rgba::argb(0xFF12_3456));
        let t = tokens_for(&scheme, false, false);
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
        let t = tokens_for(&scheme, false, false);
        assert_eq!(t.get(Role::ElevationOverlay), Rgba::argb(0xFF00_FF00));
    }

    #[test]
    fn deprecated_and_unknown_resolve_to_default() {
        let default = scheme_for(AppTheme::DEFAULT).name;
        assert_eq!(scheme_for(AppTheme::HOT_PINK).name, default);
        assert_eq!(scheme_for(AppTheme::DARK_BLUE).name, default);
        assert_eq!(scheme_for(AppTheme::BLUE).name, default);
    }

    #[test]
    fn all_eighteen_static_schemes_are_reachable() {
        assert_eq!(SCHEMES.len(), 18);
        let names: Vec<&str> = [
            AppTheme::DEFAULT, AppTheme::CLOUDFLARE, AppTheme::COTTONCANDY, AppTheme::DOOM,
            AppTheme::GREEN_APPLE, AppTheme::LAVENDER, AppTheme::MATRIX, AppTheme::MIDNIGHT_DUSK,
            AppTheme::MOCHA, AppTheme::SAPPHIRE, AppTheme::NORD, AppTheme::STRAWBERRY_DAIQUIRI,
            AppTheme::TAKO, AppTheme::TEALTURQUOISE, AppTheme::TIDAL_WAVE, AppTheme::YINYANG,
            AppTheme::YOTSUBA, AppTheme::MONOCHROME,
        ]
        .into_iter()
        .map(|a| scheme_for(a).name)
        .collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 18);
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
    /// never fail the test; the test only checks that every pair of every scheme was measured.
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
        for scheme in SCHEMES.iter() {
            for dark in [false, true] {
                let tokens = tokens_for(scheme, dark, false);
                for (fg, bg) in PAIRS {
                    let ratio = contrast(tokens.get(fg), tokens.get(bg));
                    measured += 1;
                    assert!(ratio.is_finite() && ratio >= 1.0, "{} {fg:?}/{bg:?}", scheme.name);
                    if ratio < 4.5 {
                        warnings.push(format!(
                            "{} {} {fg:?} on {bg:?}: {ratio:.2}:1",
                            scheme.name,
                            if dark { "dark" } else { "light" }
                        ));
                    }
                }
            }
        }
        assert_eq!(measured, 18 * 2 * PAIRS.len());
        for warning in &warnings {
            eprintln!("contrast warning (below 4.5:1): {warning}");
        }
        eprintln!("contrast report: {} of {measured} pairs below AA", warnings.len());
    }
}
