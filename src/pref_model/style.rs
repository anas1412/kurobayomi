//! Settings-page CSS. The values come from the constants in `widgets/mod.rs`; the flash colours
//! follow the theme through `@accent_bg_color`.

use std::cell::Cell;


pub const CSS: &str = "\
.pref-title { font-size: 16px; }
.pref-flash { background-color: alpha(@accent_bg_color, 0.12); transition: background-color 200ms ease; }
.pref-flash-off { background-color: transparent; }
.pref-error { color: @error_color; }
.pref-slider-pill { border-radius: 999px; padding: 2px 10px; }
.theme-card { border: 4px solid transparent; border-radius: 17px; padding: 4px; min-width: 114px; }
.theme-card.selected { border-color: @accent_color; }
.theme-button { padding: 0; min-height: 0; min-width: 0; }
";

thread_local! {
    static INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// Adds [`CSS`] to the default display once. Safe to call from every page build.
pub fn install() {
    if INSTALLED.with(|i| i.replace(true)) {
        return;
    }
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

#[cfg(test)]
mod tests {
    use super::CSS;

    #[test]
    fn css_defines_the_flash_and_title_classes() {
        for class in [".pref-title", ".pref-flash {", ".pref-flash-off"] {
            assert!(CSS.contains(class), "missing {class}");
        }
    }
}
