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
const GTK_NAMED: [(&str, Role); 17] = [
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

/// A CSS rule with an undefined `@kuro_*` name, or unbalanced braces.
#[derive(Debug, PartialEq, Eq)]
pub enum CssError {
    /// The selector that uses an undefined token, and the token name.
    UndefinedToken { selector: String, token: String },
    /// The braces do not balance.
    UnbalancedBraces,
}

impl std::fmt::Display for CssError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CssError::UndefinedToken { selector, token } => {
                write!(f, "rule `{selector}` uses undefined token `@{token}`")
            }
            CssError::UnbalancedBraces => write!(f, "unbalanced braces in generated CSS"),
        }
    }
}

/// Checks that every `@kuro_*` reference is defined, and that braces balance. It does not parse
/// GTK CSS; `CssProvider` does that at runtime. Failures name the offending selector (F-49-02).
pub fn check(css: &str) -> Result<(), CssError> {
    let mut defined: Vec<&str> = Vec::new();
    for line in css.lines() {
        if let Some(name) = line
            .strip_prefix("@define-color ")
            .and_then(|rest| rest.split_whitespace().next())
        {
            defined.push(name);
        }
    }
    let mut depth: i32 = 0;
    for ch in css.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return Err(CssError::UnbalancedBraces);
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(CssError::UnbalancedBraces);
    }
    for block in css.split('}') {
        let Some((selector, body)) = block.split_once('{') else {
            continue;
        };
        let selector = selector.trim().lines().last().unwrap_or("").trim();
        for token in tokens_in(body) {
            if !defined.contains(&token) {
                return Err(CssError::UndefinedToken {
                    selector: selector.to_string(),
                    token: token.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// The `kuro_*` names referenced as `@kuro_name` in a rule body.
fn tokens_in(body: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(pos) = rest.find("@kuro_") {
        let after = &rest[pos + 1..];
        let end = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        out.push(&after[..end]);
        rest = &after[end..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::resolve::{scheme_for, tokens_for};
    use crate::theme::schemes::SCHEMES;
    use kuro_prefs::ui::AppTheme;

    #[test]
    fn every_scheme_and_mode_passes_the_check() {
        for scheme in SCHEMES.iter() {
            for dark in [false, true] {
                for amoled in [false, true] {
                    let tokens = tokens_for(scheme, dark, amoled);
                    for system in [false, true] {
                        let css = render(&tokens, system);
                        assert_eq!(check(&css), Ok(()), "{} dark={dark} system={system}", scheme.name);
                    }
                }
            }
        }
    }

    #[test]
    fn all_nineteen_palettes_pass_with_the_default_mode() {
        let mut count = 0;
        for app in [
            AppTheme::DEFAULT,
            AppTheme::CLOUDFLARE,
            AppTheme::COTTONCANDY,
            AppTheme::DOOM,
            AppTheme::GREEN_APPLE,
            AppTheme::LAVENDER,
            AppTheme::MATRIX,
            AppTheme::MIDNIGHT_DUSK,
            AppTheme::MOCHA,
            AppTheme::SAPPHIRE,
            AppTheme::NORD,
            AppTheme::STRAWBERRY_DAIQUIRI,
            AppTheme::TAKO,
            AppTheme::TEALTURQUOISE,
            AppTheme::TIDAL_WAVE,
            AppTheme::YINYANG,
            AppTheme::YOTSUBA,
            AppTheme::MONOCHROME,
        ] {
            let css = render(&tokens_for(scheme_for(app), false, false), false);
            assert_eq!(check(&css), Ok(()));
            count += 1;
        }
        assert_eq!(count, 18);
    }

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
        assert!(css.contains("window {"));
    }

    #[test]
    fn check_names_the_selector_with_the_missing_token() {
        let css = "@define-color kuro_a rgba(0, 0, 0, 1);\n.kuro-x { color: @kuro_b; }\n";
        assert_eq!(
            check(css),
            Err(CssError::UndefinedToken {
                selector: ".kuro-x".into(),
                token: "kuro_b".into()
            })
        );
    }

    #[test]
    fn check_rejects_unbalanced_braces() {
        assert_eq!(check(".a { color: red;"), Err(CssError::UnbalancedBraces));
        assert_eq!(check(".a { color: red; } }"), Err(CssError::UnbalancedBraces));
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

    /// TEST-49-06: the generated CSS parses under `gtk::CssProvider`. Needs a display, and GTK binds
    /// to the first thread that initialises it, so it must not share a run with the other GTK tests:
    /// `cargo test -p kurobayomi theme::css -- --ignored` (skips itself when there is no display).
    #[test]
    #[ignore = "needs a display; run alone: cargo test -p kurobayomi generated_css_parses -- --ignored"]
    fn generated_css_parses_under_css_provider() {
        if gtk::init().is_err() {
            eprintln!("no display: skipping CssProvider parse");
            return;
        }
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
    }

    #[test]
    fn role_count_matches_the_palette() {
        let t = tokens_for(scheme_for(AppTheme::DEFAULT), false, false);
        let css = render(&t, true);
        let defines = css.lines().filter(|l| l.starts_with("@define-color kuro_")).count();
        assert_eq!(defines, super::super::tokens::ROLE_COUNT);
    }
}
