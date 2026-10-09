//! The `kb-*` component classes and their rules (`p2-rust-design.md` section 5.5), the base
//! stylesheet, and the pure alpha and padding helpers of T-49b-028 and T-49b-029.

use std::fmt::Write as _;

use super::tokens::{Role, Tokens, DISABLED_ALPHA, PAD_SMALL, SECONDARY_ALPHA};

/// The base stylesheet: GTK named colours and static rules, loaded before the palette.
pub const BASE_CSS: &str = include_str!("style.css");

/// CSS class names for the Kurobayomi component classes.
pub mod class {
    pub const SURFACE: &str = "kb-surface";
    pub const SELECTED: &str = "kb-selected";
    pub const SECONDARY_ITEM: &str = "kb-secondary-item";
    pub const HEADER: &str = "kb-header";
    pub const ACTIVE: &str = "kb-active";
    pub const PILL: &str = "kb-pill";
    pub const COVER_PLACEHOLDER: &str = "kb-cover-placeholder";
}

/// Alpha of the selected-row tint: 0.16 in dark, 0.22 in light (`selectedBackground`, M1).
pub fn selected_alpha(dark: bool) -> f32 {
    if dark { 0.16 } else { 0.22 }
}

/// Opacity of a list item. A disabled item uses only the disabled alpha, never both (M5).
pub fn item_opacity(enabled: bool, secondary: bool) -> f32 {
    if !enabled {
        DISABLED_ALPHA
    } else if secondary {
        SECONDARY_ALPHA
    } else {
        1.0
    }
}

/// Padding per edge, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// `topSmallPaddingValues`: the top edge equals the small padding, the other edges are zero.
pub fn top_small_padding() -> Edges {
    Edges { top: PAD_SMALL, ..Edges::default() }
}

/// The dark-aware rules for the GTK-theme choice: the selection tint keeps the GTK theme's own
/// selected colour, and the active tint follows the resolved dark flag (T-49b-010, T-49b-029).
pub fn system_rules(dark: bool) -> String {
    let active = Tokens::active_for(dark).to_css();
    format!(
        ".{} {{ background-color: alpha(@theme_selected_bg_color, {}); }}\n.{} {{ color: {active}; }}\n",
        class::SELECTED,
        selected_alpha(dark),
        class::ACTIVE
    )
}

/// The palette-dependent `kb-*` rules, appended to the generated palette stylesheet.
pub fn palette_rules(tokens: &Tokens) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        ".{} {{ transition: background-color 120ms cubic-bezier(0.4, 0, 0.2, 1); }}",
        class::SURFACE
    );
    let _ = writeln!(
        out,
        ".{}:active {{ transition-duration: 150ms; transition-timing-function: cubic-bezier(0.4, 0, 0.6, 1); }}",
        class::SURFACE
    );
    let _ = writeln!(out, ".{}:hover {{ transition-duration: 120ms; }}", class::SURFACE);
    let _ = writeln!(
        out,
        ".{} {{ background-color: {}; }}",
        class::SELECTED,
        tokens.get(Role::Secondary).to_css_with_alpha(selected_alpha(tokens.dark))
    );
    let _ = writeln!(
        out,
        ".{} {{ font-weight: 600; color: {}; }}",
        class::HEADER,
        tokens.get(Role::OnSurfaceVariant).to_css()
    );
    let _ = writeln!(out, ".{} {{ color: {}; }}", class::ACTIVE, tokens.active().to_css());
    let pill_alpha = if tokens.dark { 0.12 } else { 0.08 };
    let _ = writeln!(
        out,
        ".{} {{ background-color: {}; border-radius: 999px; padding: 1px 6px; font-size: 10pt; margin-left: 4px; }}",
        class::PILL,
        tokens.get(Role::OnBackground).to_css_with_alpha(pill_alpha)
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::resolve::{scheme_for, tokens_for};
    use crate::theme::tokens::Rgba;
    use kuro_prefs::ui::AppTheme;

    #[test]
    fn selected_tint_alpha_follows_the_dark_flag() {
        assert_eq!(selected_alpha(true), 0.16);
        assert_eq!(selected_alpha(false), 0.22);
        let light = tokens_for(scheme_for(AppTheme::CLOUDFLARE), false, false);
        let dark = tokens_for(scheme_for(AppTheme::CLOUDFLARE), true, false);
        assert!(palette_rules(&light).contains(".kb-selected { background-color: rgba("));
        assert!(palette_rules(&light).contains(", 0.22)"));
        assert!(palette_rules(&dark).contains(", 0.16)"));
    }

    #[test]
    fn system_rules_follow_the_dark_flag() {
        let dark = system_rules(true);
        let light = system_rules(false);
        assert!(dark.contains("alpha(@theme_selected_bg_color, 0.16)"));
        assert!(light.contains("alpha(@theme_selected_bg_color, 0.22)"));
        assert!(dark.contains(&format!(".kb-active {{ color: {}; }}", Rgba::argb(0xFFFF_EB3B).to_css())));
        assert!(light.contains(&format!(".kb-active {{ color: {}; }}", Rgba::argb(0xFFFF_C107).to_css())));
    }

    #[test]
    fn base_stylesheet_leaves_dark_aware_rules_to_the_generated_css() {
        assert!(!BASE_CSS.contains(".kb-active") && !BASE_CSS.contains(".kb-selected"));
    }

    #[test]
    fn disabled_item_uses_one_alpha_only() {
        assert_eq!(item_opacity(false, true), DISABLED_ALPHA);
        assert_eq!(item_opacity(false, false), DISABLED_ALPHA);
        assert_eq!(item_opacity(true, true), SECONDARY_ALPHA);
        assert_eq!(item_opacity(true, false), 1.0);
    }

    #[test]
    fn top_small_padding_is_only_on_top() {
        assert_eq!(top_small_padding(), Edges { top: 8.0, right: 0.0, bottom: 0.0, left: 0.0 });
    }

    #[test]
    fn active_tint_rule_uses_the_dark_flag() {
        let dark = tokens_for(scheme_for(AppTheme::DEFAULT), true, false);
        let light = tokens_for(scheme_for(AppTheme::DEFAULT), false, false);
        assert!(palette_rules(&dark).contains(&format!(".kb-active {{ color: {}; }}", Rgba::argb(0xFFFF_EB3B).to_css())));
        assert!(palette_rules(&light).contains(&format!(".kb-active {{ color: {}; }}", Rgba::argb(0xFFFF_C107).to_css())));
    }

    #[test]
    fn base_stylesheet_names_only_gtk_default_colours() {
        let allowed = [
            "theme_bg_color",
            "theme_fg_color",
            "theme_base_color",
            "theme_selected_bg_color",
            "theme_selected_fg_color",
        ];
        let mut rest = BASE_CSS;
        while let Some(pos) = rest.find('@') {
            let after = &rest[pos + 1..];
            let end = after.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(after.len());
            assert!(allowed.contains(&&after[..end]), "unexpected @{}", &after[..end]);
            rest = &after[end..];
        }
        assert_eq!(BASE_CSS.matches('{').count(), BASE_CSS.matches('}').count());
    }

    #[test]
    fn palette_rules_reference_no_undefined_tokens() {
        let tokens = tokens_for(scheme_for(AppTheme::DOOM), true, true);
        let css = crate::theme::css::render(&tokens, false);
        assert_eq!(crate::theme::css::check(&css), Ok(()));
        assert!(css.contains(".kb-pill"));
    }
}
