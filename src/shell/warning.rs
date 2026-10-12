//! The warning banner (Kotlin `WarningBanner`, T-36a-069): error colour, 16 px padding, centred body text. The WebView
//! Cloudflare notice (T-36a-050) uses it.

/// The theme's `bodyMedium` text style class, applied to the banner text.
pub const BODY_MEDIUM: &str = "kuro-body-medium";

/// The CSS class of the banner. Its colours and 16 px padding are in `theme/style.css`.
pub const CLASS: &str = "kuro-warning-banner";

/// The banner for `text`. The text wraps at the width of its parent, so a narrow window never scrolls sideways.
pub fn widget(text: &str) -> gtk::Label {
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
    fn the_banner_text_uses_the_body_medium_style_the_theme_defines() {
        assert!(crate::theme::tokens::TYPOGRAPHY.iter().any(|t| t.class == BODY_MEDIUM));
    }
}
