//! Tonal elevation: the tint Material 3 mixes over a surface at a given elevation
//! (`surfaceColorAtElevation`, T-49b-009). Pure, no GTK.
//!
//! Compose skips the mix at 0 dp and when the surface is not the scheme's own surface colour; a
//! custom colour is never tinted. The `tint` flag replaces that colour comparison.

use super::tokens::Rgba;

/// The tint opacity for an elevation in dp: `((4.5 * ln(dp + 1)) + 2) / 100`.
pub fn tint_alpha(dp: f32) -> f32 {
    ((4.5 * (dp + 1.0).ln()) + 2.0) / 100.0
}

/// The surface colour at `dp` elevation: `surface_tint` composited over `surface` with `tint_alpha`
/// (source-over). Zero, negative or NaN elevation and `tint == false` return `surface` unchanged.
pub fn tint_at(surface: Rgba, surface_tint: Rgba, dp: f32, tint: bool) -> Rgba {
    // ponytail: f32::max maps NaN to 0, so a bad elevation becomes "no tint" without a branch.
    let dp = dp.max(0.0);
    if !tint || dp == 0.0 {
        return surface;
    }
    let alpha = tint_alpha(dp);
    let mix = |s: u8, t: u8| (f32::from(s) * (1.0 - alpha) + f32::from(t) * alpha).round() as u8;
    Rgba {
        r: mix(surface.r, surface_tint.r),
        g: mix(surface.g, surface_tint.g),
        b: mix(surface.b, surface_tint.b),
        a: surface.a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACE: u32 = 0xFFFE_FBFF;
    const TINT: u32 = 0xFF00_58CA;

    fn at(dp: f32) -> Rgba {
        tint_at(Rgba::argb(SURFACE), Rgba::argb(TINT), dp, true)
    }

    // Vectors from the Compose 4.2 check: #FEFBFF over #0058CA.
    #[test]
    fn three_dp_matches_the_compose_vector() {
        assert_eq!(at(3.0), Rgba::argb(0xFFE9_EEFB));
    }

    // Table 4.2 of the edge-case file: light and dark defaults at 1, 3 and 6 dp.
    #[test]
    fn default_scheme_vectors() {
        let light = |dp| tint_at(Rgba::argb(0xFFFE_FBFF), Rgba::argb(0xFF00_58CA), dp, true);
        assert_eq!(light(1.0), Rgba::argb(0xFFF1_F3FC));
        assert_eq!(light(3.0), Rgba::argb(0xFFE9_EEFB));
        assert_eq!(light(6.0), Rgba::argb(0xFFE3_E9F9));
        let dark = |dp| tint_at(Rgba::argb(0xFF1B_1B1F), Rgba::argb(0xFFB0_C6FF), dp, true);
        assert_eq!(dark(3.0), Rgba::argb(0xFF27_2931));
        assert_eq!(dark(6.0), Rgba::argb(0xFF2B_2D37));
    }

    #[test]
    fn zero_dp_is_the_surface() {
        assert_eq!(at(0.0), Rgba::argb(SURFACE));
    }

    #[test]
    fn negative_input_is_clamped_to_zero() {
        assert_eq!(at(-4.0), Rgba::argb(SURFACE));
        assert_eq!(at(f32::NAN), Rgba::argb(SURFACE));
    }

    #[test]
    fn a_surface_without_tint_is_never_tinted() {
        let out = tint_at(Rgba::argb(SURFACE), Rgba::argb(TINT), 3.0, false);
        assert_eq!(out, Rgba::argb(SURFACE));
    }

    #[test]
    fn alpha_follows_the_formula() {
        assert!((tint_alpha(3.0) - 0.082_38).abs() < 1e-4);
        assert!((tint_alpha(0.0) - 0.02).abs() < 1e-6);
    }
}
