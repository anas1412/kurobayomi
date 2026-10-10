//! WebView helpers that need no WebKit (`WebViewUtil.kt`, T-13a-052, T-12a-041): the Chrome major version parse and
//! the settings every app view gets. `getInferredUserAgent` is dropped: it had no callers (T-12a-037). The WebKitGTK
//! host applies the settings to a view (T-12a-017); the functions here are the part that can be tested without a
//! display.

use std::sync::LazyLock;

use regex::Regex;

/// `.*Chrome/(\d+)\..*`: the Chrome major version, from the last `Chrome/N.` in the user agent.
static CHROME_MAJOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r".*Chrome/(\d+)\..*").unwrap());

/// The Chrome major version in a user agent, or 0 when it has none. A 0 counts as outdated.
pub fn parse_chrome_major(ua: &str) -> u32 {
    CHROME_MAJOR
        .captures(ua)
        .and_then(|caps| caps[1].parse().ok())
        .unwrap_or(0)
}

/// The settings `WebViewUtil.setDefaultSettings` gives every view. The Android zoom and viewport flags have no
/// desktop meaning and are not here. The cache policy is the session default, so it is not here either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewSettings {
    pub javascript: bool,
    pub local_storage: bool,
    pub database: bool,
    /// Accept third-party cookies (`acceptThirdPartyCookies`).
    pub third_party_cookies: bool,
}

/// The settings of every app view.
pub const APP_VIEW_SETTINGS: ViewSettings = ViewSettings {
    javascript: true,
    local_storage: true,
    database: true,
    third_party_cookies: true,
};

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 7 Build/TQ3A.230805.001) AppleWebKit/537.36 \
        (KHTML, like Gecko) Version/4.0 Chrome/116.0.0.0 Mobile Safari/537.36";

    #[test]
    fn the_chrome_major_is_read_from_chrome_and_is_zero_when_absent() {
        assert_eq!(parse_chrome_major("Chrome/116.0.0.0"), 116);
        assert_eq!(parse_chrome_major(SAMPLE), 116);
        assert_eq!(parse_chrome_major("Firefox/136.0"), 0);
        assert_eq!(parse_chrome_major("Chrome/abc."), 0);
    }

    #[test]
    fn every_app_view_enables_javascript_storage_and_third_party_cookies() {
        assert_eq!(
            APP_VIEW_SETTINGS,
            ViewSettings {
                javascript: true,
                local_storage: true,
                database: true,
                third_party_cookies: true,
            }
        );
    }
}
