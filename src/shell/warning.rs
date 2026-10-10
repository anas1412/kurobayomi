//! The warning banner (Kotlin `WarningBanner`, T-36a-069): error colour, 16 px padding, centred body text. The WebView
//! Cloudflare notice (T-36a-050) uses it.

use std::cell::Cell;

/// Padding on every side, in logical pixels.
pub const PADDING_PX: i32 = 16;

/// The theme's `bodyMedium` text style class, applied to the banner text.
pub const BODY_MEDIUM: &str = "kuro-body-medium";

/// The CSS class of the banner.
pub const CLASS: &str = "kuro-warning-banner";

/// The banner colours: the error role and its text role.
pub fn css() -> String {
    format!(".{CLASS} {{ background-color: @kuro_error; color: @kuro_on_error; padding: {PADDING_PX}px; }}\n")
}

thread_local! {
    static CSS_LOADED: Cell<bool> = const { Cell::new(false) };
}

fn ensure_css() {
    if CSS_LOADED.with(|done| done.replace(true)) {
        return;
    }
    let Some(display) = gtk::gdk::Display::default() else { return };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css());
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

/// The banner for `text`. The text wraps at the width of its parent, so a narrow window never scrolls sideways.
pub fn widget(text: &str) -> gtk::Label {
    ensure_css();
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .hexpand(true)
        .css_classes([CLASS, BODY_MEDIUM])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_uses_the_error_role_with_sixteen_pixel_padding() {
        assert_eq!(PADDING_PX, 16);
        assert!(css().contains(".kuro-warning-banner { background-color: @kuro_error; color: @kuro_on_error; padding: 16px; }"));
    }

    #[test]
    fn the_banner_text_uses_the_body_medium_style_the_theme_defines() {
        assert!(crate::theme::tokens::TYPOGRAPHY.iter().any(|t| t.class == BODY_MEDIUM));
    }
}
