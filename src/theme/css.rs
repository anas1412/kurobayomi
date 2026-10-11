//! Tokens to GTK CSS text, and the offline validity check (T-49a-012, T-49a-061).
//!
//! The text has four parts: `@define-color kuro_*` for every role; the GTK named-colour override
//! block (skipped for the System scheme); the component classes; and nothing else.

use std::fmt::Write as _;

use super::components::classes as cls;
use super::style;
use super::tokens::{
    Role, Tokens, TYPOGRAPHY, DIALOG_PAD, PAD_EXTRA_LARGE, PAD_EXTRA_SMALL, PAD_MEDIUM, PAD_SMALL,
    RADIUS_EXTRA_LARGE, RADIUS_SMALL, SETTINGS_PAD_H, SETTINGS_PAD_V,
};

/// The GTK named colours that the scheme overrides, and the role each one takes.
const GTK_NAMED: [(&str, Role); 18] = [
    ("window_bg_color", Role::Background),
    ("window_fg_color", Role::OnBackground),
    ("view_bg_color", Role::Surface),
    ("view_fg_color", Role::OnSurface),
    ("headerbar_bg_color", Role::SurfaceContainer),
    ("headerbar_fg_color", Role::OnSurface),
    ("card_bg_color", Role::SurfaceContainerLow),
    ("card_fg_color", Role::OnSurface),
    ("popover_bg_color", Role::SurfaceContainerHigh),
    ("accent_color", Role::Primary),
    ("accent_bg_color", Role::Primary),
    ("accent_fg_color", Role::OnPrimary),
    ("theme_selected_bg_color", Role::Primary),
    ("destructive_color", Role::Error),
    ("destructive_bg_color", Role::Error),
    ("destructive_fg_color", Role::OnError),
    ("borders", Role::Outline),
    ("dark_fill_color", Role::SurfaceVariant),
];

/// Renders the stylesheet for a resolved palette. `system_scheme` skips the named-colour block.
pub fn render(tokens: &Tokens, system_scheme: bool) -> String {
    let mut out = String::with_capacity(12 * 1024);
    for role in Role::ALL {
        let _ = writeln!(
            out,
            "@define-color kuro_{} {};",
            role.css_name(),
            tokens.get(role).to_css()
        );
    }
    if !system_scheme {
        for (name, role) in GTK_NAMED {
            let _ = writeln!(out, "@define-color {name} @kuro_{};", role.css_name());
        }
        let _ = writeln!(
            out,
            "window {{ background-color: @kuro_background; color: @kuro_on_background; }}"
        );
    }
    out.push_str(&typography_rules());
    out.push_str(&component_rules());
    out.push_str(&style::palette_rules(tokens));
    out
}

/// One `.kuro-<style>` rule per `TYPOGRAPHY` entry, generated from the tokens.
fn typography_rules() -> String {
    let mut out = String::new();
    for t in TYPOGRAPHY.iter() {
        let _ = write!(out, ".{} {{ font-size: {}em;", t.class, t.size_em);
        if let Some(weight) = t.weight {
            let _ = write!(out, " font-weight: {weight};");
        }
        if let Some(line) = t.line_height_px {
            let _ = write!(out, " line-height: {line}px;");
        }
        out.push_str(" }\n");
    }
    out
}

/// The `.kuro-*` component classes. Geometry comes from the token constants.
fn component_rules() -> String {
    format!(
        "\
.{badge} {{ background-color: @kuro_secondary; color: @kuro_on_secondary; padding: 1px 3px; border-radius: {rxs}px; font-size: 0.75em; font-weight: 500; }}
.{badge}.{downloaded} {{ background-color: @kuro_tertiary; color: @kuro_on_tertiary; }}
.{pill} {{ background-color: @kuro_surface_container_high; color: @kuro_on_surface; border-radius: 999px; padding: 1px 6px; margin-left: {pxs}px; }}
.{group} {{ color: @kuro_on_surface_variant; font-weight: 600; font-size: 0.875em; padding: {pv}px {pm}px; }}
.{card} {{ background-color: @kuro_surface_container_low; border-radius: {rxl}px; margin: {pv}px {pm}px; padding: {pm}px; }}
.{card_title} {{ font-weight: 500; font-size: 0.875em; margin: 0 {pxl}px; }}
.{dialog} {{ background-color: @kuro_surface_container_high; border-radius: {rxl}px; padding: {dp}px; }}
.{dialog_text} {{ color: @kuro_on_surface_variant; font-size: 0.875em; }}
.{ring} {{ color: @kuro_primary; }}
.{row} {{ padding: {sv}px {sh}px; }}
.{heading} {{ font-weight: 600; padding: {sv}px {sh}px; color: @kuro_on_surface_variant; }}
.{icon} {{ color: @kuro_primary; }}
.{check_disabled}:disabled {{ opacity: 0.38; }}
",
        badge = cls::BADGE,
        downloaded = cls::BADGE_DOWNLOADED,
        pill = cls::PILL,
        group = cls::GROUP_HEADER,
        card = cls::SECTION_CARD,
        card_title = cls::SECTION_TITLE,
        dialog = cls::DIALOG,
        dialog_text = cls::DIALOG_TEXT,
        ring = cls::PROGRESS_RING,
        row = cls::SETTINGS_ROW,
        heading = cls::SETTINGS_HEADING,
        icon = cls::SETTINGS_ICON,
        check_disabled = cls::CHECK_DISABLED,
        rxs = RADIUS_SMALL,
        rxl = RADIUS_EXTRA_LARGE,
        pxs = PAD_EXTRA_SMALL,
        pxl = PAD_EXTRA_LARGE,
        pv = PAD_SMALL,
        pm = PAD_MEDIUM,
        dp = DIALOG_PAD,
        sv = SETTINGS_PAD_V,
        sh = SETTINGS_PAD_H,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::resolve::{scheme_for, tokens_for};
    use crate::theme::schemes::SCHEMES;
    use kuro_prefs::ui::AppTheme;

    #[test]
    fn output_stays_under_twenty_kilobytes() {
        for scheme in SCHEMES.iter() {
            let css = render(&tokens_for(scheme, true, true), false);
            assert!(css.len() < 20 * 1024, "{} is {} bytes", scheme.name, css.len());
        }
    }

    #[test]
    fn system_scheme_emits_no_named_colour_overrides() {
        let t = tokens_for(scheme_for(AppTheme::DEFAULT), false, false);
        let css = render(&t, true);
        assert!(!css.contains("accent_color"));
        assert!(css.contains("@define-color kuro_primary"));
        assert!(css.contains(".kuro-badge"));
    }

    #[test]
    fn named_colours_are_overridden_for_a_scheme() {
        let t = tokens_for(scheme_for(AppTheme::CLOUDFLARE), false, false);
        let css = render(&t, false);
        assert!(css.contains("@define-color accent_color @kuro_primary;"));
        assert!(css.contains("@define-color theme_selected_bg_color @kuro_primary;"));
        assert!(css.contains("window {"));
    }

    #[test]
    fn component_classes_use_the_spec_geometry() {
        let css = component_rules();
        assert!(css.contains("padding: 8px 16px"), "group header padding");
        assert!(css.contains("border-radius: 28px"), "section card radius");
        assert!(css.contains("padding: 10px 24px"), "settings row padding");
    }

    #[test]
    fn typography_block_is_generated_from_the_tokens() {
        let css = typography_rules();
        assert!(css.contains(".kuro-body-medium { font-size: 0.875em; line-height: 20px; }"));
        assert!(css.contains(".kuro-label-large { font-size: 0.875em; font-weight: 500; }"));
        assert!(css.contains(".kuro-title-medium { font-size: 1em; font-weight: 500; line-height: 24px; }"));
        assert!(css.contains(".kuro-headline-small { font-size: 1.5em; line-height: 32px; }"));
        assert!(css.contains(".kuro-body-small { font-size: 0.75em; line-height: 16px; }"));
        assert_eq!(css.lines().count(), TYPOGRAPHY.len());
        let all = render(&tokens_for(scheme_for(AppTheme::DEFAULT), false, false), true);
        assert!(all.contains(&css));
    }

    /// TEST-49-06: the generated CSS parses under `gtk::CssProvider`. Needs a display, and skips
    /// itself without one.
    #[test]
    fn generated_css_parses_under_css_provider() {
        crate::testing::with_display(|| {
            for (app, dark, system) in [
                (AppTheme::DEFAULT, false, true),
                (AppTheme::DOOM, true, false),
                (AppTheme::CLOUDFLARE, false, false),
            ] {
                let css = render(&tokens_for(scheme_for(app), dark, false), system);
                if let Err(message) = super::super::apply::build_provider(&css) {
                    panic!("CSS did not parse: {message}");
                }
            }
        });
    }

    #[test]
    fn role_count_matches_the_palette() {
        let t = tokens_for(scheme_for(AppTheme::DEFAULT), false, false);
        let css = render(&t, true);
        let defines = css.lines().filter(|l| l.starts_with("@define-color kuro_")).count();
        assert_eq!(defines, super::super::tokens::ROLE_COUNT);
    }
}
