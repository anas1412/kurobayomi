//! Monet: the dynamic scheme seeded from the desktop accent (T-49a-005, T-49a-006, T-49a-007).
//!
//! Seed sources, in order: the Settings portal `accent-color`, then the KDE `AccentColor` entry of
//! the `kdeglobals` file (its `Colors:Header` background as the last resort). With neither,
//! Monet is unavailable: the picker hides it and a stored Monet resolves to System (E-49-06, E-49-41).
//!
//! Generation is the `SchemeContent` variant (spec 2021) of the `material-colors` crate, a port of
//! `material-color-utilities`. Its `tertiary_container` is read as `tertiary_container`, which
//! avoids the Android defect at `MonetColorScheme.kt:97` (E-49-07).

use std::cell::{Cell, RefCell};

use gtk::gio::{self, prelude::*};
use gtk::glib;
use material_colors::color::Rgb;
use material_colors::hct::Hct;
use material_colors::scheme::Scheme;
use material_colors::scheme::variant::SchemeContent;

use super::tokens::{Role, Rgba, Tokens, ROLE_COUNT};

const PORTAL_BUS: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS_IFACE: &str = "org.freedesktop.portal.Settings";
const APPEARANCE: &str = "org.freedesktop.appearance";
/// Seed reads give up after this long, and Monet stays unavailable (F-49-03).
pub const SEED_TIMEOUT_MS: i32 = 2000;
const KDE_FILE: &str = "kdeglobals";

thread_local! {
    /// The seed read at startup, or `None` while unavailable. Read by `resolve`.
    static SEED: RefCell<Option<Rgba>> = const { RefCell::new(None) };
    /// The portal contrast (0 normal, 1 high), as `SchemeContent` takes it.
    static CONTRAST: Cell<f64> = const { Cell::new(0.0) };
}

/// The seed in use, if Monet can be shown.
pub fn seed() -> Option<Rgba> {
    SEED.with(|s| *s.borrow())
}

/// The contrast level in use: 0.0 normal, 1.0 high.
pub fn contrast() -> f64 {
    CONTRAST.with(Cell::get)
}

/// True when a seed was read. Gates the picker entry and the default (T-49a-007).
pub fn available() -> bool {
    seed().is_some()
}

/// Stores the seed and contrast. `None` keeps Monet unavailable.
pub fn store(seed: Option<Rgba>, contrast: f64) {
    SEED.with(|s| *s.borrow_mut() = seed);
    CONTRAST.with(|c| c.set(clamp_contrast(contrast)));
}

/// Clamps a contrast to the supported range (E-49-08). NaN reads as normal contrast.
pub fn clamp_contrast(value: f64) -> f64 {
    if value.is_nan() { 0.0 } else { value.clamp(0.0, 1.0) }
}

/// A portal `accent-color` `(ddd)`: channels in `0..=1`. NaN or out-of-range values are rejected
/// (E-49-40). The alpha channel does not exist in this spec, so the seed is opaque.
pub fn seed_from_portal(rgb: (f64, f64, f64)) -> Option<Rgba> {
    let mut packed = 0u32;
    for channel in [rgb.0, rgb.1, rgb.2] {
        if !(0.0..=1.0).contains(&channel) {
            return None;
        }
        packed = (packed << 8) | (channel * 255.0).round() as u32;
    }
    Some(Rgba::argb(0xFF00_0000 | packed))
}

/// The seed from a `kdeglobals` text: `[General] AccentColor=r,g,b`, else
/// `[Colors:Header] BackgroundNormal=r,g,b`. Malformed or missing values are `None`.
pub fn seed_from_kde(ini: &str) -> Option<Rgba> {
    let value = ini_value(ini, "General", "AccentColor")
        .or_else(|| ini_value(ini, "Colors:Header", "BackgroundNormal"))?;
    let mut packed = 0u32;
    let mut parts = 0;
    for part in value.split(',') {
        let channel: u32 = part.trim().parse().ok()?;
        if channel > 255 {
            return None;
        }
        packed = (packed << 8) | channel;
        parts += 1;
    }
    (parts == 3).then(|| Rgba::argb(0xFF00_0000 | packed))
}

/// The value of `key` in `[section]`, found with a plain line scan (no INI crate needed).
fn ini_value<'a>(ini: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut current = "";
    for line in ini.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            current = name;
        } else if current == section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim());
                }
            }
        }
    }
    None
}

/// Generates the full 36-role table (plus the two aliases) for a seed, light or dark.
/// Output is opaque; `scrim` is black. AMOLED is applied by the caller after this.
pub fn generate(seed: Rgba, is_dark: bool, contrast: f64) -> Tokens {
    let rgb = Rgb::from_u32(u32::from(seed.r) << 16 | u32::from(seed.g) << 8 | u32::from(seed.b));
    let hct = Hct::new(rgb);
    let s: Scheme = SchemeContent::new(hct, is_dark, Some(clamp_contrast(contrast))).scheme.into();

    let mut colors = [Rgba::argb(0xFF00_0000); ROLE_COUNT];
    let mut put = |role: Role, c: Rgb| colors[role as usize] = opaque(c);
    put(Role::Primary, s.primary);
    put(Role::OnPrimary, s.on_primary);
    put(Role::PrimaryContainer, s.primary_container);
    put(Role::OnPrimaryContainer, s.on_primary_container);
    put(Role::InversePrimary, s.inverse_primary);
    put(Role::Secondary, s.secondary);
    put(Role::OnSecondary, s.on_secondary);
    put(Role::SecondaryContainer, s.secondary_container);
    put(Role::OnSecondaryContainer, s.on_secondary_container);
    put(Role::Tertiary, s.tertiary);
    put(Role::OnTertiary, s.on_tertiary);
    put(Role::TertiaryContainer, s.tertiary_container);
    put(Role::OnTertiaryContainer, s.on_tertiary_container);
    put(Role::Background, s.background);
    put(Role::OnBackground, s.on_background);
    put(Role::Surface, s.surface);
    put(Role::OnSurface, s.on_surface);
    put(Role::SurfaceVariant, s.surface_variant);
    put(Role::OnSurfaceVariant, s.on_surface_variant);
    put(Role::SurfaceTint, s.surface_tint);
    put(Role::InverseSurface, s.inverse_surface);
    put(Role::InverseOnSurface, s.inverse_on_surface);
    put(Role::Error, s.error);
    put(Role::OnError, s.on_error);
    put(Role::ErrorContainer, s.error_container);
    put(Role::OnErrorContainer, s.on_error_container);
    put(Role::Outline, s.outline);
    put(Role::OutlineVariant, s.outline_variant);
    put(Role::SurfaceBright, s.surface_bright);
    put(Role::SurfaceDim, s.surface_dim);
    put(Role::SurfaceContainer, s.surface_container);
    put(Role::SurfaceContainerHigh, s.surface_container_high);
    put(Role::SurfaceContainerHighest, s.surface_container_highest);
    put(Role::SurfaceContainerLow, s.surface_container_low);
    put(Role::SurfaceContainerLowest, s.surface_container_lowest);
    put(Role::PrimaryInverseAlias, s.inverse_primary);
    put(Role::ElevationOverlay, s.primary);
    colors[Role::Scrim as usize] = Rgba::argb(0xFF00_0000);
    Tokens { colors, amoled: false, dark: is_dark }
}

fn opaque(c: Rgb) -> Rgba {
    Rgba::argb(0xFF00_0000 | c.as_u32())
}

/// Unboxes portal variants (`ReadOne` wraps the value in `v`) until a `T` is found.
fn unwrap_value<T: glib::variant::FromVariant>(mut v: glib::Variant) -> Option<T> {
    for _ in 0..3 {
        if let Some(x) = v.get::<T>() {
            return Some(x);
        }
        v = v.as_variant()?;
    }
    None
}

/// One `ReadOne` call on the Settings portal. `None` on any error or timeout.
async fn read_portal_one(namespace: &str, key: &str) -> Option<glib::Variant> {
    let conn = gio::bus_get_future(gio::BusType::Session).await.ok()?;
    let params = (namespace, key).to_variant();
    let reply = conn
        .call_future(
            Some(PORTAL_BUS),
            PORTAL_PATH,
            SETTINGS_IFACE,
            "ReadOne",
            Some(&params),
            None,
            gio::DBusCallFlags::NONE,
            SEED_TIMEOUT_MS,
        )
        .await
        .ok()?;
    Some(reply.child_value(0))
}

/// Reads the accent from the portal, else from `kdeglobals`, and the contrast from the portal,
/// then stores them and calls `on_done`. A missing source is info-level; nothing shows to the user.
pub fn read_seed<F: FnOnce(Option<Rgba>) + 'static>(on_done: F) {
    glib::spawn_future_local(async move {
        let portal = match read_portal_one(APPEARANCE, "accent-color").await {
            Some(v) => unwrap_value::<(f64, f64, f64)>(v).and_then(seed_from_portal),
            None => None,
        };
        let seed = portal.or_else(kde_seed);
        if seed.is_none() {
            tracing::info!("no accent colour from the settings portal or kdeglobals; Monet unavailable");
        }
        let contrast = match read_portal_one(APPEARANCE, "contrast").await {
            Some(v) => unwrap_value::<u32>(v).map_or(0.0, |n| if n == 1 { 1.0 } else { 0.0 }),
            None => 0.0,
        };
        store(seed, contrast);
        on_done(seed);
    });
}

/// The KDE fallback, read from the user config directory.
fn kde_seed() -> Option<Rgba> {
    let path = glib::user_config_dir().join(KDE_FILE);
    seed_from_kde(&std::fs::read_to_string(path).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_accent_rounds_to_the_nearest_channel_value() {
        // TEST-49-23
        let seed = seed_from_portal((0.1176, 0.5333, 0.8980));
        assert_eq!(seed, Some(Rgba::argb(0xFF1E_88E5)));
    }

    #[test]
    fn portal_accent_out_of_range_or_nan_is_rejected() {
        // E-49-40
        assert_eq!(seed_from_portal((1.5, 0.0, 0.0)), None);
        assert_eq!(seed_from_portal((-0.1, 0.0, 0.0)), None);
        assert_eq!(seed_from_portal((f64::NAN, 0.0, 0.0)), None);
    }

    #[test]
    fn kde_accent_colour_key_is_read() {
        // TEST-49-24
        let ini = "[General]\nColorScheme=Breeze\nAccentColor=30,136,229\n";
        assert_eq!(seed_from_kde(ini), Some(Rgba::argb(0xFF1E_88E5)));
    }

    #[test]
    fn kde_falls_back_to_the_header_background() {
        let ini = "[Colors:Header]\nBackgroundNormal=30,136,229\n";
        assert_eq!(seed_from_kde(ini), Some(Rgba::argb(0xFF1E_88E5)));
    }

    #[test]
    fn kde_missing_or_malformed_values_are_none() {
        assert_eq!(seed_from_kde(""), None);
        assert_eq!(seed_from_kde("[General]\nAccentColor=30,136\n"), None);
        assert_eq!(seed_from_kde("[General]\nAccentColor=30,136,300\n"), None);
        assert_eq!(seed_from_kde("[General]\nAccentColor=a,b,c\n"), None);
        assert_eq!(seed_from_kde("[Other]\nAccentColor=1,2,3\n"), None);
    }

    #[test]
    fn contrast_is_clamped_to_the_supported_range() {
        // E-49-08
        assert_eq!(clamp_contrast(2.0), 1.0);
        assert_eq!(clamp_contrast(-3.0), 0.0);
        assert_eq!(clamp_contrast(f64::NAN), 0.0);
        assert_eq!(clamp_contrast(0.5), 0.5);
    }

    #[test]
    fn unavailable_without_a_stored_seed() {
        store(None, 0.0);
        assert!(!available());
        store(Some(Rgba::argb(0xFF00_00FF)), 0.0);
        assert!(available());
        store(None, 0.0);
    }

    #[test]
    fn generated_primary_matches_the_reference_vector() {
        // TEST-49-08: vector from material-color-utilities (crate tests), seed 0x0000FF, light, contrast 0.
        let t = generate(Rgba::argb(0xFF00_00FF), false, 0.0);
        assert_eq!(t.get(Role::Primary), Rgba::argb(0xFF00_01BB));
        assert_eq!(t.get(Role::PrimaryContainer), Rgba::argb(0xFF00_00FF));
    }

    /// TEST-49-08: every one of the 36 roles for seed 0xFF1E88E5, light and dark, contrast 0 and 1,
    /// against vectors frozen in `fixtures/monet_vectors.txt` (from an independent port of
    /// material-color-utilities).
    #[test]
    fn all_36_roles_match_the_frozen_reference_vectors() {
        let mut rows = 0;
        for line in include_str!("fixtures/monet_vectors.txt").lines().filter(|l| !l.starts_with('#')) {
            let mut cols = line.split_whitespace();
            let dark = cols.next() == Some("dark");
            let contrast: f64 = cols.next().and_then(|c| c.parse().ok()).expect("contrast column");
            let expected: Vec<&str> = cols.next().expect("role column").split(',').collect();
            assert_eq!(expected.len(), 36);
            let t = generate(Rgba::argb(0xFF1E_88E5), dark, contrast);
            for (role, hex) in Role::ALL.iter().zip(&expected) {
                let want = Rgba::argb(0xFF00_0000 | u32::from_str_radix(hex, 16).expect("hex"));
                assert_eq!(t.get(*role), want, "{role:?} dark={dark} contrast={contrast}");
            }
            rows += 1;
        }
        assert_eq!(rows, 4);
    }

    #[test]
    fn tertiary_container_comes_from_the_container_role() {
        // TEST-49-09 / E-49-07: the reference scheme's tertiary_container, not its tertiary.
        let seed = Rgba::argb(0xFF00_00FF);
        let rgb = Rgb::from_u32(0x0000FF);
        let reference: Scheme = SchemeContent::new(Hct::new(rgb), false, Some(0.0)).scheme.into();
        let t = generate(seed, false, 0.0);
        assert_eq!(t.get(Role::TertiaryContainer), opaque(reference.tertiary_container));
        assert_ne!(reference.tertiary_container, reference.tertiary);
    }

    #[test]
    fn generated_table_is_opaque_with_black_scrim() {
        let t = generate(Rgba::argb(0xFF1E_88E5), true, 1.0);
        for role in Role::ALL {
            assert_eq!(t.get(role).a, 0xFF, "{role:?}");
        }
        assert_eq!(t.get(Role::Scrim), Rgba::argb(0xFF00_0000));
        assert!(t.dark);
    }
}
