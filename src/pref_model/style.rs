//! Settings-page CSS. Theme colours only: the flash and the selected theme card use the theme's selected colour
//! (`@theme_selected_bg_color`), like the rest of the app. Row titles keep the theme's body size.

use std::cell::Cell;


/// The CSS.
pub fn css() -> String {
    format!(
        "\
.pref-flash {{ background-color: alpha(@theme_selected_bg_color, 0.18); transition: background-color 200ms ease; }}
.pref-flash-off {{ background-color: transparent; }}
.pref-error {{ color: @error_color; }}
.tracker-check {{ color: @success_color; }}
.pref-slider-pill {{ border-radius: 999px; padding: 2px 10px; }}
"
    )
}

thread_local! {
    static INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// Adds [`css`] to the default display once. Safe to call from every page build.
pub fn install() {
    if INSTALLED.with(|i| i.replace(true)) {
        return;
    }
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css());
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

#[cfg(test)]
mod tests {
    use super::css;

    #[test]
    fn the_tracker_check_uses_the_theme_success_colour() {
        let css = css();
        assert!(css.contains(".tracker-check { color: @success_color; }"));
        assert!(!css.contains('#'), "no hard-coded colours");
    }

    #[test]
    fn css_defines_the_flash_classes_in_theme_colours() {
        let css = css();
        for class in [".pref-flash {", ".pref-flash-off"] {
            assert!(css.contains(class), "missing {class}");
        }
        assert!(!css.contains("@accent"), "the theme's named colours only");
        assert!(!css.contains(".pref-title {"), "row titles keep the theme's body size");
    }
}
