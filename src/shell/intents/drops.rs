//! Android features this port does not carry, each with the reason and what replaces it (T-36a-064, T-36a-068). The
//! drop decisions live here so the code and its tests name them; a feature listed here has no route or screen.

/// One dropped feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drop {
    pub feature: &'static str,
    pub reason: &'static str,
    /// What the user keeps instead. Empty when nothing replaces the feature.
    pub replacement: &'static str,
}

pub const DROPS: [Drop; 4] = [
    Drop {
        feature: "assist content (AssistContentScreen)",
        reason: "the Android assistant has no Linux equivalent",
        replacement: "the current window URL in the WebView page header",
    },
    Drop {
        feature: "install-unknown-apps permission row",
        reason: "Linux installs no packages from the app, so no install permission is asked for",
        replacement: "none; the user loses nothing",
    },
    Drop {
        feature: "runtime notification permission row",
        reason: "desktop notifications need no runtime permission",
        replacement: "the desktop notification status row (T-36a-027)",
    },
    Drop {
        feature: "battery-optimisation exemption row",
        reason: "Linux has no battery-optimisation exemption to request",
        replacement: "none; the user loses nothing",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_drop_names_its_reason_and_what_replaces_it_or_says_nothing_is_lost() {
        for drop in DROPS {
            assert!(!drop.feature.is_empty() && !drop.reason.is_empty(), "{}", drop.feature);
            assert!(!drop.replacement.is_empty(), "{}", drop.feature);
        }
    }

    #[test]
    fn the_notification_permission_is_replaced_by_the_status_row() {
        let notification = DROPS.iter().find(|d| d.feature.contains("notification permission")).unwrap();
        assert!(notification.replacement.contains("T-36a-027"));
    }

    #[test]
    fn the_assist_content_is_replaced_by_the_window_url_in_the_webview_header() {
        let assist = DROPS.iter().find(|d| d.feature.contains("assist content")).unwrap();
        assert!(assist.replacement.contains("WebView page header"));
    }
}
