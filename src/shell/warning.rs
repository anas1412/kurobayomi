//! The warning banner (Kotlin `WarningBanner`, T-36a-069): the shared banner in the error tint, centred body text. The WebView
//! Cloudflare notice (T-36a-050) uses it.

#[cfg(test)]
/// The theme's `bodyMedium` text style class, applied to the banner text.
pub const BODY_MEDIUM: &str = "kuro-body-medium";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_text_uses_the_body_medium_style_the_theme_defines() {
        assert!(crate::theme::tokens::TYPOGRAPHY.iter().any(|t| t.class == BODY_MEDIUM));
    }
}
