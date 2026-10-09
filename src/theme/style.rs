//! The `kb-*` component classes and their rules (`p2-rust-design.md` section 5.5), the base
//! stylesheet, and the pure alpha and padding helpers of T-49b-028 and T-49b-029.

use std::fmt::Write as _;

use super::tokens::{self as tk, Role, Tokens, DISABLED_ALPHA, PAD_SMALL, Rgba, SECONDARY_ALPHA};

/// `EasingEmphasizedCubicBezier(0.2, 0, 0, 1)` (`p1-behavior.md` 22).
const EMPHASIZED: &str = "cubic-bezier(0.2, 0, 0, 1)";

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
    pub const STATE_LAYER: &str = "kb-state-layer";
    pub const DRAGGED: &str = "kb-dragged";
    pub const FAST_THUMB: &str = "kb-fast-thumb";
    pub const FAST_SHOWN: &str = "kb-fast-shown";
    pub const BUTTON_FILLED: &str = "kb-button-filled";
    pub const BUTTON_TEXT: &str = "kb-button-text";
    pub const FAB: &str = "kb-fab";
    pub const FAB_EXPANDED: &str = "kb-fab-expanded";
    pub const FAB_ROW: &str = "kb-fab-row";
    pub const FAB_LABEL: &str = "kb-fab-label";
    pub const SHEET: &str = "kb-sheet";
    pub const SHEET_TABLET: &str = "kb-sheet-tablet";
    pub const SHEET_SHOWN: &str = "kb-sheet-shown";
    pub const LINK_ICON: &str = "kb-link-icon";
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
/// selected colour, the active tint follows the resolved dark flag (T-49b-010, T-49b-029), and the
/// pill takes a GTK-theme stand-in for `surface_container_high` (T-49a-034).
pub fn system_rules(dark: bool) -> String {
    let active = Tokens::active_for(dark).to_css();
    format!(
        ".{} {{ background-color: alpha(@theme_selected_bg_color, {}); }}\n.{} {{ color: {active}; }}\n.{} {{ background-color: mix(@theme_bg_color, @theme_fg_color, 0.1); }}\n",
        class::SELECTED,
        selected_alpha(dark),
        class::ACTIVE,
        class::PILL
    )
}

/// The fast-scroll thumb (T-49a-031): hidden by default, shown with a class, and faded by a CSS
/// transition of `SCROLLBAR_FADE_MS`. Its colour is the primary accent.
pub fn fast_thumb_rules() -> String {
    format!(
        ".{thumb} {{ opacity: 0; border-radius: 6px; background-color: @accent_color; transition: opacity {fade}ms linear; }}\n.{thumb}.{shown} {{ opacity: 1; transition: none; }}\n",
        thumb = class::FAST_THUMB,
        shown = class::FAST_SHOWN,
        fade = crate::theme::tokens::SCROLLBAR_FADE_MS,
    )
}

/// One rule that gives a single fast-scroll thumb its own colour (`thumb_class` is unique to it).
/// The two-class selector outranks the default thumb rule whatever the provider order.
pub fn fast_thumb_color_rule(thumb_class: &str, color: Rgba) -> String {
    format!(".{}.{thumb_class} {{ background-color: {}; }}\n", class::FAST_THUMB, color.to_css())
}

/// The rules that depend on the system dark flag or the tokens of the widget layer, added to the
/// tint provider for every choice.
pub fn widget_rules(system_dark: bool) -> String {
    format!("{}{}{}", ripple_rules(system_dark), fast_thumb_rules(), widget_static_rules())
}

/// The widget classes of `widgets/`. They use GTK named colours only, so they follow the GTK theme
/// and a picked scheme alike (the scheme overrides the named colours).
pub fn widget_static_rules() -> String {
    format!(
        "\
.{filled} {{ background-color: @accent_bg_color; color: @accent_fg_color; min-width: 58px; min-height: 40px; border-radius: 999px; padding: 0 24px; }}
.{filled}:hover {{ box-shadow: 0 1px 2px alpha(black, 0.3); }}
.{filled}:disabled {{ background-color: alpha(@accent_bg_color, 0.12); color: alpha(@window_fg_color, 0.38); }}
.{text} {{ background: none; color: @accent_color; padding: 0 12px; }}
.{text}:disabled {{ color: alpha(@window_fg_color, 0.38); }}
.{fab} {{ min-width: 56px; min-height: 56px; border-radius: 16px; background-color: @accent_bg_color; color: @accent_fg_color; transition: min-width {w}ms {ease}; }}
.{fab}.{fab_expanded} {{ min-width: 80px; }}
.{fab_row} {{ padding-left: 0; transition: padding-left {pad_out}ms {ease}; }}
.{fab}.{fab_expanded} .{fab_row} {{ padding-left: 12px; transition: padding-left {pad_in}ms {ease}; }}
.{fab_label} {{ opacity: 0; transition: opacity {fade_out}ms linear; }}
.{fab}.{fab_expanded} .{fab_label} {{ opacity: 1; transition: opacity {fade_in}ms linear {fade_delay}ms; }}
.{sheet} {{ background-color: @popover_bg_color; border-radius: 28px 28px 0 0; padding: 24px; }}
.{sheet_tablet} {{ border-radius: 28px; opacity: 0; transition: opacity {sheet_ms}ms linear; }}
.{sheet_tablet}.{sheet_shown} {{ opacity: 1; }}
.{link} {{ color: @accent_color; }}
.{pill} {{ padding: 1px 6px; border-radius: 999px; }}
",
        filled = class::BUTTON_FILLED,
        text = class::BUTTON_TEXT,
        fab = class::FAB,
        fab_expanded = class::FAB_EXPANDED,
        fab_row = class::FAB_ROW,
        fab_label = class::FAB_LABEL,
        w = tk::FAB_WIDTH_MS,
        ease = EMPHASIZED,
        pad_in = tk::FAB_START_PAD_EXPAND_MS,
        pad_out = tk::FAB_START_PAD_COLLAPSE_MS,
        fade_in = tk::FAB_EXPAND_FADE_MS,
        fade_delay = tk::FAB_EXPAND_FADE_DELAY_MS,
        fade_out = tk::FAB_COLLAPSE_FADE_MS,
        sheet = class::SHEET,
        sheet_tablet = class::SHEET_TABLET,
        sheet_shown = class::SHEET_SHOWN,
        sheet_ms = tk::SHEET_ANIM_MS,
        link = class::LINK_ICON,
        pill = class::PILL,
    )
}

/// The player state layer (T-49a-010): `RIPPLE_ALPHA` over hover, focus-visible, active and drag.
/// White in the system dark mode and black in the system light mode, never the app scheme.
pub fn ripple_rules(system_dark: bool) -> String {
    let rgb = if system_dark { "255, 255, 255" } else { "0, 0, 0" };
    format!(
        ".{layer}:hover, .{layer}:focus-visible, .{layer}:active, .{layer}.{dragged} {{ background-color: rgba({rgb}, {alpha}); }}\n",
        layer = class::STATE_LAYER,
        dragged = class::DRAGGED,
        alpha = crate::theme::tokens::RIPPLE_ALPHA,
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
    let _ = writeln!(
        out,
        ".{} {{ background-color: {}; color: {}; }}",
        class::PILL,
        tokens.get(Role::SurfaceContainerHigh).to_css(),
        tokens.get(Role::OnSurface).to_css()
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

    // T-49a-034: the pill geometry is always on; the colour is the token in a palette and a GTK
    // mix in System mode.
    #[test]
    fn pill_geometry_is_always_on_and_colour_is_surface_container_high() {
        let rules = widget_rules(false);
        assert!(rules.contains(".kb-pill { padding: 1px 6px; border-radius: 999px; }"));
        assert!(system_rules(true).contains(".kb-pill { background-color: mix(@theme_bg_color"));
        for dark in [false, true] {
            let t = tokens_for(scheme_for(AppTheme::CLOUDFLARE), dark, false);
            let want = format!(".kb-pill {{ background-color: {};", t.get(Role::SurfaceContainerHigh).to_css());
            assert!(palette_rules(&t).contains(&want), "dark={dark}");
        }
    }

    // T-49a-052 / TEST-49-21: the FAB rules carry the source timings.
    #[test]
    fn fab_rules_use_the_source_timings() {
        let css = widget_static_rules();
        assert!(css.contains("transition: min-width 500ms cubic-bezier(0.2, 0, 0, 1)"));
        assert!(css.contains(".kb-fab-row { padding-left: 0; transition: padding-left 900ms"));
        assert!(css.contains(".kb-fab.kb-fab-expanded .kb-fab-row { padding-left: 12px; transition: padding-left 300ms"));
        assert!(css.contains(".kb-fab-label { opacity: 0; transition: opacity 100ms linear; }"));
        assert!(css.contains(".kb-fab.kb-fab-expanded .kb-fab-label { opacity: 1; transition: opacity 200ms linear 100ms; }"));
    }

    // T-49a-023: the tablet sheet fades in and out over the sheet duration.
    #[test]
    fn tablet_sheet_fades_over_350_ms() {
        let css = widget_static_rules();
        assert!(css.contains(".kb-sheet-tablet { border-radius: 28px; opacity: 0; transition: opacity 350ms linear; }"));
        assert!(css.contains(".kb-sheet-tablet.kb-sheet-shown { opacity: 1; }"));
    }

    #[test]
    fn fast_thumb_colour_rule_targets_one_thumb() {
        let rule = fast_thumb_color_rule("kb-thumb-7", Rgba::argb(0xFF1E_88E5));
        assert_eq!(rule, ".kb-fast-thumb.kb-thumb-7 { background-color: rgba(30, 136, 229, 1); }\n");
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

#[cfg(test)]
mod ripple_tests {
    use super::{fast_thumb_rules, ripple_rules};

    #[test]
    fn fast_thumb_fades_over_the_scrollbar_duration() {
        let css = fast_thumb_rules();
        assert!(css.contains("transition: opacity 250ms"));
        assert!(css.contains(".kb-fast-thumb.kb-fast-shown"));
    }

    #[test]
    fn ripple_is_white_in_system_dark_and_black_in_system_light() {
        // T-49a-010 / MANUAL-49-07
        assert!(ripple_rules(true).contains("rgba(255, 255, 255, 0.1)"));
        assert!(ripple_rules(false).contains("rgba(0, 0, 0, 0.1)"));
    }

    #[test]
    fn ripple_covers_hover_focus_active_and_drag() {
        let css = ripple_rules(true);
        for state in [":hover", ":focus-visible", ":active", ".kb-dragged"] {
            assert!(css.contains(state), "{state}");
        }
    }
}


#[cfg(test)]
mod parse_tests {
    /// The widget and system rules parse under `gtk::CssProvider`. Needs a display; run alone:
    /// `cargo test -p kurobayomi widget_rules_parse -- --ignored`.
    #[test]
    #[ignore = "needs a display; run alone: cargo test -p kurobayomi widget_rules_parse -- --ignored"]
    fn widget_rules_parse_under_css_provider() {
        if gtk::init().is_err() {
            eprintln!("no display: skipping");
            return;
        }
        for dark in [false, true] {
            let css = format!("{}{}", super::system_rules(dark), super::widget_rules(dark));
            if let Err(message) = crate::theme::apply::build_provider(&css) {
                panic!("CSS did not parse: {message}");
            }
        }
    }
}
