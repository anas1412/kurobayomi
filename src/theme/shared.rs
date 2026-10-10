//! Colours that are not Material roles: the cover placeholder, the dividers and the AMOLED surfaces
//! (T-49b-040). Android sources: presentation-core `values/colors.xml` and `values-night/colors.xml`.
//!
//! The splash colours are the `.kb-splash` rules in `style.css` (T-49ba-020). The `accent_blue`
//! accent is dropped (T-49ba-038): no notification or shortcut sets an accent colour.

use super::tokens::Rgba;

/// Fill of a cover that is missing or failed to load (`cover_placeholder`).
pub const COVER_PLACEHOLDER: Rgba = Rgba::argb(0x1F88_8888);
/// Divider in light mode (`md_black_1000_12`).
pub const DIVIDER_LIGHT: Rgba = Rgba::argb(0x1F00_0000);
/// Divider in dark mode (`md_white_1000_12`).
pub const DIVIDER_DARK: Rgba = Rgba::argb(0x1FFF_FFFF);

pub const AMOLED_BACKGROUND: Rgba = Rgba::argb(0xFF00_0000);
pub const AMOLED_ON_BACKGROUND: Rgba = Rgba::argb(0xFFFF_FFFF);
pub const AMOLED_SURFACE: Rgba = Rgba::argb(0xFF00_0000);
pub const AMOLED_ON_SURFACE: Rgba = Rgba::argb(0xFFFF_FFFF);
pub const AMOLED_SURFACE_CONTAINER: Rgba = Rgba::argb(0xFF0C_0C0C);
pub const AMOLED_SURFACE_CONTAINER_HIGH: Rgba = Rgba::argb(0xFF13_1313);
pub const AMOLED_SURFACE_CONTAINER_HIGHEST: Rgba = Rgba::argb(0xFF1B_1B1B);

/// The divider colour for a mode.
pub fn divider(dark: bool) -> Rgba {
    if dark { DIVIDER_DARK } else { DIVIDER_LIGHT }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The literals of the Android resources, parsed the same way the XML is read.
    #[test]
    fn values_match_the_android_resources() {
        assert_eq!(COVER_PLACEHOLDER, Rgba::parse_hex("#1F888888").unwrap());
        assert_eq!(DIVIDER_LIGHT, Rgba::parse_hex("#1F000000").unwrap());
        assert_eq!(DIVIDER_DARK, Rgba::parse_hex("#1FFFFFFF").unwrap());
        assert_eq!(AMOLED_SURFACE_CONTAINER, Rgba::parse_hex("#0C0C0C").unwrap());
        assert_eq!(AMOLED_SURFACE_CONTAINER_HIGH, Rgba::parse_hex("#131313").unwrap());
        assert_eq!(AMOLED_SURFACE_CONTAINER_HIGHEST, Rgba::parse_hex("#1B1B1B").unwrap());
    }

    #[test]
    fn divider_follows_the_mode() {
        assert_eq!(divider(false), DIVIDER_LIGHT);
        assert_eq!(divider(true), DIVIDER_DARK);
    }

    #[test]
    fn cover_placeholder_alpha_is_the_stylesheet_alpha() {
        // style.css writes the same fill as rgba(136, 136, 136, 0.1216): 0x1F rounds to that alpha.
        assert_eq!(COVER_PLACEHOLDER.a, 0x1F);
        assert!(super::super::style::BASE_CSS.contains("rgba(136, 136, 136, 0.1216)"));
    }
}
