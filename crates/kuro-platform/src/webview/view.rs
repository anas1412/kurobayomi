//! WebView helpers that need no WebKit (`WebViewUtil.kt`, T-13a-052, T-12a-041): the user agent transform, the
//! Chrome major version parse, and the settings every app view gets. The WebKitGTK host applies the settings to a
//! view (T-12a-017); the functions here are the part that can be tested without a display.

use std::sync::LazyLock;

use regex::Regex;

/// `; Android .*?\)`: the Android platform part of a user agent, up to its first `)`.
static ANDROID_PLATFORM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"; Android .*?\)").unwrap());
/// `Version/.* Chrome/`: the Android WebView `Version/x` token, up to `Chrome/`.
static VERSION_TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Version/.* Chrome/").unwrap());
/// `.*Chrome/(\d+)\..*`: the Chrome major version, from the last `Chrome/N.` in the user agent.
static CHROME_MAJOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r".*Chrome/(\d+)\..*").unwrap());

/// The user agent the Android app shows sites: the platform part becomes `Android 10; K)` and the `Version/x` token
/// is removed. This is `WebViewUtil.getInferredUserAgent`, kept for parity (OQ-13a-4, see DECISIONS).
pub fn infer_user_agent(default_ua: &str) -> String {
    let platform = ANDROID_PLATFORM.replace_all(default_ua, "; Android 10; K)");
    VERSION_TOKEN.replace_all(&platform, "Chrome/").into_owned()
}

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
    fn the_android_platform_and_the_version_token_are_rewritten() {
        assert_eq!(
            infer_user_agent(SAMPLE),
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Mobile Safari/537.36"
        );
    }

    #[test]
    fn a_user_agent_without_the_android_parts_is_unchanged() {
        let desktop = "Mozilla/5.0 (X11; Linux x86_64) Gecko/20100101 Firefox/136.0";
        assert_eq!(infer_user_agent(desktop), desktop);
    }

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
