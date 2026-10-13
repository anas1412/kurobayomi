//! The page error card (T-44a-064, T-44a-065, UI §1.1, B§8.5, E-42, E-84). A page that cannot be shown keeps this card
//! in place of its image: the message, Retry (which loads the page again) and, only for an http or https image url,
//! Open in browser. The rules are pure; [`card`] is the thin widget around them.

use gtk::prelude::*;

use super::strings;

/// The message of a failed page (`reader_error`).
pub const PAGE_ERROR_TEXT: &str = "The image couldn't be loaded";

/// Height of the webtoon's error card as a share of the viewport (B§8.5).
pub const WEBTOON_CARD_RATIO: f64 = 0.8;

/// Whether an image url may be opened in the default browser. Only http and https (E-84). Any other scheme, an empty
/// url, a url without a host, and no url at all hide the button.
pub fn can_open_in_browser(url: Option<&str>) -> bool {
    let Some(url) = url.map(str::trim) else { return false };
    let lower = url.to_ascii_lowercase();
    let host = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"));
    host.is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/') && !rest.chars().any(char::is_whitespace))
}

/// The height of a webtoon error card: 0.8 times the viewport, never negative.
pub fn webtoon_card_height(viewport: i32) -> i32 {
    (f64::from(viewport.max(0)) * WEBTOON_CARD_RATIO) as i32
}

/// The error card for a page: the message, Retry, and Open in browser when [`can_open_in_browser`] allows it.
pub fn card(on_retry: impl Fn() + 'static, image_url: Option<String>) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root.set_halign(gtk::Align::Center);
    root.set_valign(gtk::Align::Center);
    root.set_margin_start(16);
    root.set_margin_end(16);
    let message = gtk::Label::new(Some(PAGE_ERROR_TEXT));
    message.set_justify(gtk::Justification::Center);
    message.set_wrap(true);
    root.append(&message);
    let retry = gtk::Button::with_label(strings::RETRY);
    retry.connect_clicked(move |_| on_retry());
    root.append(&retry);
    if can_open_in_browser(image_url.as_deref()) {
        let open = gtk::Button::with_label(strings::ACTION_OPEN_IN_BROWSER);
        let url = image_url.unwrap_or_default();
        open.connect_clicked(move |button| {
            let window = button.root().and_downcast::<gtk::Window>();
            crate::ui::open_link(&url, window.as_ref());
        });
        root.append(&open);
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_and_https_urls_get_the_open_in_browser_button() {
        assert!(can_open_in_browser(Some("https://example.org/a.png")));
        assert!(can_open_in_browser(Some("HTTP://example.org/a.png")));
        assert!(!can_open_in_browser(Some("file:///home/a.png")));
        assert!(!can_open_in_browser(Some("ftp://example.org/a.png")));
        assert!(!can_open_in_browser(Some("javascript:alert(1)")));
        assert!(!can_open_in_browser(Some("https://")));
        assert!(!can_open_in_browser(Some("https:///path")));
        assert!(!can_open_in_browser(Some("https://exa mple.org")));
        assert!(!can_open_in_browser(None));
    }

    #[test]
    fn the_webtoon_card_is_four_fifths_of_the_viewport() {
        assert_eq!(webtoon_card_height(1000), 800);
        assert_eq!(webtoon_card_height(0), 0);
        assert_eq!(webtoon_card_height(-5), 0);
    }

    #[test]
    fn the_message_is_the_upstream_text() {
        assert_eq!(PAGE_ERROR_TEXT, "The image couldn't be loaded");
    }
}
