//! Android features this port does not carry, each with the reason and what replaces it (T-36a-064, T-36a-068). The
//! drop decisions live here so the code and its tests name them; a feature listed here has no route or screen.

#[cfg(test)]
/// One dropped feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drop {
    pub feature: &'static str,
    pub reason: &'static str,
    /// What the user keeps instead. Empty when nothing replaces the feature.
    pub replacement: &'static str,
}

#[cfg(test)]
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
}
