//! Colours that are not Material roles: the cover placeholder, the dividers and the AMOLED surfaces
//! (T-49b-040). Android sources: presentation-core `values/colors.xml` and `values-night/colors.xml`.
//!
//! The splash colours are the `.kb-splash` rules in `style.css` (T-49ba-020). The `accent_blue`
//! accent is dropped (T-49ba-038): no notification or shortcut sets an accent colour.

use super::tokens::Rgba;

pub const AMOLED_BACKGROUND: Rgba = Rgba::argb(0xFF00_0000);
pub const AMOLED_ON_BACKGROUND: Rgba = Rgba::argb(0xFFFF_FFFF);
pub const AMOLED_SURFACE: Rgba = Rgba::argb(0xFF00_0000);
pub const AMOLED_ON_SURFACE: Rgba = Rgba::argb(0xFFFF_FFFF);
pub const AMOLED_SURFACE_CONTAINER: Rgba = Rgba::argb(0xFF0C_0C0C);
pub const AMOLED_SURFACE_CONTAINER_HIGH: Rgba = Rgba::argb(0xFF13_1313);
pub const AMOLED_SURFACE_CONTAINER_HIGHEST: Rgba = Rgba::argb(0xFF1B_1B1B);
