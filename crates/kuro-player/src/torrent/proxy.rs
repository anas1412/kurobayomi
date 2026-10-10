//! Proxy modes of the torrent server (T-45a-077; `TorrentPreferences.kt:46-51`, area 13 section 6).

/// The stored `ProxyMode` (`pref_torrserver_proxymode`, stored by name). An unknown name is `None`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxyMode {
    #[default]
    None,
    Tracker,
    Peers,
    Full,
}

impl ProxyMode {
    pub fn from_pref(name: &str) -> Self {
        match name {
            "Tracker" => Self::Tracker,
            "Peers" => Self::Peers,
            "Full" => Self::Full,
            _ => Self::None,
        }
    }

    /// The value the server takes. `None` sends `tracker`, the same value as `Tracker`, as the source does.
    pub const fn server_value(self) -> &'static str {
        match self {
            Self::None | Self::Tracker => "tracker",
            Self::Peers => "peers",
            Self::Full => "full",
        }
    }
}

const SCHEMES: [&str; 6] = ["http", "https", "socks4", "socks4a", "socks5", "socks5h"];

/// A proxy URL the server accepts: one of the schemes and a host.
pub fn valid_proxy_url(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| {
        SCHEMES.contains(&u.scheme()) && u.host_str().is_some_and(|host| !host.is_empty())
    })
}

/// The `(mode, proxy url)` pair the server starts with. The URL is sent only when the mode is not `None`.
pub fn start_arguments(mode: ProxyMode, proxy_url: &str) -> (&'static str, Option<&str>) {
    match mode {
        ProxyMode::None => (mode.server_value(), None),
        other => (other.server_value(), Some(proxy_url)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_modes_map_to_the_server_values() {
        assert_eq!(ProxyMode::None.server_value(), "tracker");
        assert_eq!(ProxyMode::Tracker.server_value(), "tracker");
        assert_eq!(ProxyMode::Peers.server_value(), "peers");
        assert_eq!(ProxyMode::Full.server_value(), "full");
        assert_eq!(ProxyMode::from_pref("Peers"), ProxyMode::Peers);
        assert_eq!(ProxyMode::from_pref("bogus"), ProxyMode::None);
    }

    #[test]
    fn the_proxy_url_is_sent_only_when_the_mode_is_not_none() {
        assert_eq!(
            start_arguments(ProxyMode::None, "socks5://h:1"),
            ("tracker", None)
        );
        assert_eq!(
            start_arguments(ProxyMode::Full, "socks5://h:1"),
            ("full", Some("socks5://h:1"))
        );
    }

    #[test]
    fn only_the_listed_schemes_with_a_host_are_valid() {
        assert!(valid_proxy_url("socks5h://proxy.example:1080"));
        assert!(valid_proxy_url("http://10.0.0.1:3128"));
        assert!(!valid_proxy_url("ftp://proxy.example"));
        assert!(!valid_proxy_url("http://"));
        assert!(!valid_proxy_url("not a url"));
    }
}
