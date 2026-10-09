//! Colour token model (Role, Rgba, Tokens) and the layout, alpha, duration and radius constants.
//!
//! Dp values are CSS px at scale 1; GTK does the scaling, so no constant is multiplied here.

/// An sRGB colour with alpha, stored as 8-bit channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    /// Parses an Android-style `0xAARRGGBB` value (the XML and Kotlin literal form).
    pub const fn argb(v: u32) -> Self {
        Rgba {
            a: (v >> 24) as u8,
            r: (v >> 16) as u8,
            g: (v >> 8) as u8,
            b: v as u8,
        }
    }

    /// Parses `#RRGGBB` (opaque) or `#AARRGGBB`. Anything else is `None`.
    pub fn parse_hex(s: &str) -> Option<Self> {
        let digits = s.strip_prefix('#')?;
        if !digits.bytes().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        match digits.len() {
            6 => u32::from_str_radix(digits, 16)
                .ok()
                .map(|v| Self::argb(0xFF00_0000 | v)),
            8 => u32::from_str_radix(digits, 16).ok().map(Self::argb),
            _ => None,
        }
    }

    /// CSS `rgba(...)` text with the given alpha (0..1) rather than the stored one.
    pub fn to_css_with_alpha(self, alpha: f32) -> String {
        format!("rgba({}, {}, {}, {})", self.r, self.g, self.b, trim_unit(alpha))
    }

    /// CSS `rgba(...)` text with alpha as a 0..1 number, for `@define-color`.
    pub fn to_css(self) -> String {
        format!(
            "rgba({}, {}, {}, {})",
            self.r,
            self.g,
            self.b,
            format_alpha(self.a)
        )
    }
}

fn format_alpha(a: u8) -> String {
    trim_unit(a as f32 / 255.0)
}

fn trim_unit(v: f32) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A colour role. The first 36 are the Material 3 roles; the last two are XML-only aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Role {
    Primary,
    OnPrimary,
    PrimaryContainer,
    OnPrimaryContainer,
    InversePrimary,
    Secondary,
    OnSecondary,
    SecondaryContainer,
    OnSecondaryContainer,
    Tertiary,
    OnTertiary,
    TertiaryContainer,
    OnTertiaryContainer,
    Background,
    OnBackground,
    Surface,
    OnSurface,
    SurfaceVariant,
    OnSurfaceVariant,
    SurfaceTint,
    InverseSurface,
    InverseOnSurface,
    Error,
    OnError,
    ErrorContainer,
    OnErrorContainer,
    Outline,
    OutlineVariant,
    Scrim,
    SurfaceBright,
    SurfaceDim,
    SurfaceContainer,
    SurfaceContainerHigh,
    SurfaceContainerHighest,
    SurfaceContainerLow,
    SurfaceContainerLowest,
    /// XML key `primaryInverse`; used as `InversePrimary` when that is absent.
    PrimaryInverseAlias,
    /// XML key `elevationOverlay`; resolves to `Primary` when absent.
    ElevationOverlay,
}

/// Number of roles; the length of every role table.
pub const ROLE_COUNT: usize = 38;

impl Role {
    /// Every role in index order (`ALL[i] as usize == i`).
    pub const ALL: [Role; ROLE_COUNT] = [
        Role::Primary,
        Role::OnPrimary,
        Role::PrimaryContainer,
        Role::OnPrimaryContainer,
        Role::InversePrimary,
        Role::Secondary,
        Role::OnSecondary,
        Role::SecondaryContainer,
        Role::OnSecondaryContainer,
        Role::Tertiary,
        Role::OnTertiary,
        Role::TertiaryContainer,
        Role::OnTertiaryContainer,
        Role::Background,
        Role::OnBackground,
        Role::Surface,
        Role::OnSurface,
        Role::SurfaceVariant,
        Role::OnSurfaceVariant,
        Role::SurfaceTint,
        Role::InverseSurface,
        Role::InverseOnSurface,
        Role::Error,
        Role::OnError,
        Role::ErrorContainer,
        Role::OnErrorContainer,
        Role::Outline,
        Role::OutlineVariant,
        Role::Scrim,
        Role::SurfaceBright,
        Role::SurfaceDim,
        Role::SurfaceContainer,
        Role::SurfaceContainerHigh,
        Role::SurfaceContainerHighest,
        Role::SurfaceContainerLow,
        Role::SurfaceContainerLowest,
        Role::PrimaryInverseAlias,
        Role::ElevationOverlay,
    ];

    /// The CSS name without the `kuro_` prefix (snake case), e.g. `surface_container_high`.
    pub const fn css_name(self) -> &'static str {
        match self {
            Role::Primary => "primary",
            Role::OnPrimary => "on_primary",
            Role::PrimaryContainer => "primary_container",
            Role::OnPrimaryContainer => "on_primary_container",
            Role::InversePrimary => "inverse_primary",
            Role::Secondary => "secondary",
            Role::OnSecondary => "on_secondary",
            Role::SecondaryContainer => "secondary_container",
            Role::OnSecondaryContainer => "on_secondary_container",
            Role::Tertiary => "tertiary",
            Role::OnTertiary => "on_tertiary",
            Role::TertiaryContainer => "tertiary_container",
            Role::OnTertiaryContainer => "on_tertiary_container",
            Role::Background => "background",
            Role::OnBackground => "on_background",
            Role::Surface => "surface",
            Role::OnSurface => "on_surface",
            Role::SurfaceVariant => "surface_variant",
            Role::OnSurfaceVariant => "on_surface_variant",
            Role::SurfaceTint => "surface_tint",
            Role::InverseSurface => "inverse_surface",
            Role::InverseOnSurface => "inverse_on_surface",
            Role::Error => "error",
            Role::OnError => "on_error",
            Role::ErrorContainer => "error_container",
            Role::OnErrorContainer => "on_error_container",
            Role::Outline => "outline",
            Role::OutlineVariant => "outline_variant",
            Role::Scrim => "scrim",
            Role::SurfaceBright => "surface_bright",
            Role::SurfaceDim => "surface_dim",
            Role::SurfaceContainer => "surface_container",
            Role::SurfaceContainerHigh => "surface_container_high",
            Role::SurfaceContainerHighest => "surface_container_highest",
            Role::SurfaceContainerLow => "surface_container_low",
            Role::SurfaceContainerLowest => "surface_container_lowest",
            Role::PrimaryInverseAlias => "primary_inverse_alias",
            Role::ElevationOverlay => "elevation_overlay",
        }
    }
}

/// A resolved palette: one colour per role plus the flags it was resolved with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tokens {
    pub colors: [Rgba; ROLE_COUNT],
    pub amoled: bool,
    pub dark: bool,
}

impl Tokens {
    pub fn get(&self, role: Role) -> Rgba {
        self.colors[role as usize]
    }

    /// The active-filter tint (Compose `ColorScheme.active`): amber in dark, darker amber in light.
    pub fn active(&self) -> Rgba {
        Self::active_for(self.dark)
    }

    /// The active tint for a dark flag, when no palette tokens exist (GTK-theme choice).
    pub fn active_for(dark: bool) -> Rgba {
        if dark {
            Rgba::argb(0xFFFF_EB3B)
        } else {
            Rgba::argb(0xFFFF_C107)
        }
    }
}

// --- Spacing, alpha, duration and radius (T-49a-051) ---

pub const PAD_EXTRA_LARGE: f32 = 32.0;
pub const PAD_LARGE: f32 = 24.0;
pub const PAD_MEDIUM: f32 = 16.0;
pub const PAD_MEDIUM_SMALL: f32 = 12.0;
pub const PAD_SMALL: f32 = 8.0;
pub const PAD_EXTRA_SMALL: f32 = 4.0;
pub const DISABLED_ALPHA: f32 = 0.38;
pub const SECONDARY_ALPHA: f32 = 0.78;
pub const ICON_STATE_LAYER: f32 = 40.0;
pub const RIPPLE_ALPHA: f32 = 0.10;
pub const SHEET_MAX_WIDTH_LANDSCAPE: f32 = 600.0;
pub const SHEET_MAX_WIDTH_PORTRAIT: f32 = 460.0;
pub const SHEET_ANIM_MS: u32 = 350;
pub const SHEET_POSITIONAL_THRESHOLD: f32 = 56.0;
pub const SHEET_VELOCITY_THRESHOLD: f32 = 125.0;
pub const THUMB_LENGTH: f32 = 48.0;
pub const THUMB_THICKNESS: f32 = 12.0;
pub const SCROLLBAR_FADE_MS: u32 = 250;
pub const SCROLLBAR_FADE_DELAY_MS: u32 = 2000;
pub const SCROLL_SAMPLE_MS: u32 = 100;
pub const WHEEL_ROWS: u32 = 3;
pub const WHEEL_SIZE: (f32, f32) = (128.0, 128.0);
pub const REPEAT_MAX_MS: u64 = 750;
pub const REPEAT_MIN_MS: u64 = 5;
pub const REPEAT_DECAY: f64 = 0.25;
pub const NUMERIC_MIN_WIDTH: f32 = 140.0;
pub const DIALOG_MIN_WIDTH: f32 = 280.0;
pub const DIALOG_MAX_WIDTH: f32 = 560.0;
pub const DIALOG_PAD: f32 = 24.0;
pub const FAB_COLLAPSED_WIDTH: f32 = 56.0;
pub const FAB_EXPANDED_MIN_WIDTH: f32 = 80.0;
pub const FAB_ICON_PAD: f32 = 12.0;
pub const FAB_TEXT_PAD: f32 = 20.0;
pub const PROGRESS_ROTATION_MS: u32 = 2000;
pub const TWO_PANEL_FIRST_MAX: f32 = 450.0;
pub const SETTINGS_PAD_H: f32 = 24.0;
pub const SETTINGS_PAD_V: f32 = 10.0;
pub const RADIUS_EXTRA_SMALL: f32 = 4.0;
pub const RADIUS_SMALL: f32 = 8.0;
pub const RADIUS_MEDIUM: f32 = 12.0;
pub const RADIUS_LARGE: f32 = 16.0;
pub const RADIUS_EXTRA_LARGE: f32 = 28.0;

/// Window-size class breakpoints, dp (`WindowSize.kt`): medium from 600, expanded from 840.
pub const MEDIUM_WIDTH_WINDOW_SIZE: f32 = 600.0;
pub const EXPANDED_WIDTH_WINDOW_SIZE: f32 = 840.0;

/// ExtendedFab timings, ms (`FloatingActionButton.kt:58-131`).
pub const FAB_WIDTH_MS: u32 = 500;
pub const FAB_START_PAD_EXPAND_MS: u32 = 300;
pub const FAB_START_PAD_COLLAPSE_MS: u32 = 900;
pub const FAB_EXPAND_FADE_MS: u32 = 200;
pub const FAB_EXPAND_FADE_DELAY_MS: u32 = 100;
pub const FAB_COLLAPSE_FADE_MS: u32 = 100;
pub const FAB_EXPAND_SIZE_MS: u32 = 500;
pub const FAB_COLLAPSE_SIZE_MS: u32 = 500;

/// One text style: the CSS class it is emitted under, `font-size` in em, an optional weight
/// (omitted means normal) and an optional `line-height` in px.
pub struct TypeStyle {
    pub class: &'static str,
    pub size_em: f32,
    pub weight: Option<u16>,
    pub line_height_px: Option<f32>,
}

/// The Material 3 styles the app uses (`p1-rust-design.md` section 5).
pub const TYPOGRAPHY: [TypeStyle; 6] = [
    TypeStyle { class: "kuro-body-medium", size_em: 0.875, weight: None, line_height_px: Some(20.0) },
    TypeStyle { class: "kuro-label-large", size_em: 0.875, weight: Some(500), line_height_px: None },
    TypeStyle { class: "kuro-title-small", size_em: 0.875, weight: Some(500), line_height_px: None },
    TypeStyle { class: "kuro-title-medium", size_em: 1.0, weight: Some(500), line_height_px: Some(24.0) },
    TypeStyle { class: "kuro-headline-small", size_em: 1.5, weight: None, line_height_px: Some(32.0) },
    TypeStyle { class: "kuro-body-small", size_em: 0.75, weight: None, line_height_px: Some(16.0) },
];

pub const EASE_EMPHASIZED: (f32, f32, f32, f32) = (0.2, 0.0, 0.0, 1.0);
pub const EASE_LINEAR: (f32, f32, f32, f32) = (0.0, 0.0, 1.0, 1.0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_splits_channels() {
        let c = Rgba::argb(0xD849_454E);
        assert_eq!((c.a, c.r, c.g, c.b), (216, 0x49, 0x45, 0x4E));
    }

    #[test]
    fn hex_six_digits_is_opaque() {
        let c = Rgba::parse_hex("#F38020").unwrap();
        assert_eq!(c, Rgba::argb(0xFFF3_8020));
    }

    #[test]
    fn hex_eight_digits_keeps_alpha() {
        assert_eq!(Rgba::parse_hex("#80112233").unwrap().a, 0x80);
    }

    #[test]
    fn hex_rejects_bad_input() {
        assert_eq!(Rgba::parse_hex("F38020"), None);
        assert_eq!(Rgba::parse_hex("#GGGGGG"), None);
        assert_eq!(Rgba::parse_hex("#12345"), None);
    }

    #[test]
    fn role_table_is_in_index_order() {
        for (i, role) in Role::ALL.iter().enumerate() {
            assert_eq!(*role as usize, i);
        }
        assert_eq!(Role::ALL.len(), ROLE_COUNT);
    }

    #[test]
    fn css_names_are_unique() {
        let mut names: Vec<&str> = Role::ALL.iter().map(|r| r.css_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ROLE_COUNT);
    }

    #[test]
    fn active_tint_follows_dark_flag() {
        let mut t = Tokens {
            colors: [Rgba::argb(0); ROLE_COUNT],
            amoled: false,
            dark: true,
        };
        assert_eq!(t.active(), Rgba::argb(0xFFFF_EB3B));
        t.dark = false;
        assert_eq!(t.active(), Rgba::argb(0xFFFF_C107));
    }

    #[test]
    fn section_25_constants_have_their_values() {
        assert_eq!((MEDIUM_WIDTH_WINDOW_SIZE, EXPANDED_WIDTH_WINDOW_SIZE), (600.0, 840.0));
        assert_eq!(FAB_WIDTH_MS, 500);
        assert_eq!((FAB_START_PAD_EXPAND_MS, FAB_START_PAD_COLLAPSE_MS), (300, 900));
        assert_eq!((FAB_EXPAND_FADE_MS, FAB_EXPAND_FADE_DELAY_MS), (200, 100));
        assert_eq!(FAB_COLLAPSE_FADE_MS, 100);
        assert_eq!((FAB_EXPAND_SIZE_MS, FAB_COLLAPSE_SIZE_MS), (500, 500));
    }

    #[test]
    fn typography_classes_are_unique_and_kuro_prefixed() {
        let mut names: Vec<&str> = TYPOGRAPHY.iter().map(|t| t.class).collect();
        assert!(names.iter().all(|n| n.starts_with("kuro-")));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TYPOGRAPHY.len());
    }

    #[test]
    fn css_alpha_is_trimmed() {
        assert_eq!(Rgba::argb(0xFF00_0000).to_css(), "rgba(0, 0, 0, 1)");
        assert_eq!(Rgba::argb(0x8000_0000).to_css(), "rgba(0, 0, 0, 0.502)");
    }
}
