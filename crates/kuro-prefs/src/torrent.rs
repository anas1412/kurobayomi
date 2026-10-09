//! `[torrent]` preferences (Aniyomi `TorrentPreferences`): the embedded torrent server's switches, port, announce
//! list and proxy. The server itself is in the torrent area; this section only holds the settings.

use crate::pref_enum;
use crate::registry::{TORRSERVER_TRACKERS, define_prefs};

pref_enum! {
    /// `pref_torrserver_proxymode`, stored as the Kotlin enum name.
    pub enum ProxyMode { None, Tracker, Peers, Full }
}

impl ProxyMode {
    /// The value sent to the server (`ProxyMode.value`). Quirk Q6: `None` sends `"tracker"`, the same as `Tracker`.
    pub fn wire_value(self) -> &'static str {
        match self {
            ProxyMode::None | ProxyMode::Tracker => "tracker",
            ProxyMode::Peers => "peers",
            ProxyMode::Full => "full",
        }
    }
}

impl ProxyMode {
    /// The proxy mode sent to the server (OQ-13a-3, deviation D-13): `None` sends no proxy mode at all, unlike
    /// [`ProxyMode::wire_value`], which keeps the Kotlin quirk of sending `"tracker"`.
    pub fn server_mode(self) -> Option<&'static str> {
        match self {
            ProxyMode::None => None,
            other => Some(other.wire_value()),
        }
    }
}

/// The port field is not a number between 0 and 65535.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPort;

/// Kotlin `toIntOrNull` then `in 0..65535`: a leading `+` or `-` is accepted by the parse, `""` and spaces are not.
pub fn validate_port(s: &str) -> Result<u16, InvalidPort> {
    s.parse::<i32>().ok().and_then(|n| u16::try_from(n).ok()).ok_or(InvalidPort)
}

/// Why a proxy URL is refused (messages `pref_player_torrents_proxy_url_invalid_uri` and `..._invalid_protocol`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyUrlError {
    /// No scheme or no host.
    InvalidUri,
    /// A scheme other than the six supported ones.
    UnsupportedProtocol(String),
}

const PROXY_SCHEMES: [&str; 6] = ["http", "https", "socks4", "socks4a", "socks5", "socks5h"];

/// `(scheme, host)` the way Android `Uri` reports them: `None` when absent.
fn scheme_and_host(url: &str) -> (Option<&str>, Option<&str>) {
    let Some((scheme, rest)) = url.split_once(':') else { return (None, None) };
    let valid_scheme = scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c));
    if !valid_scheme {
        return (None, None);
    }
    let Some(after) = rest.strip_prefix("//") else { return (Some(scheme), None) };
    let authority = after.split(['/', '?', '#']).next().unwrap_or("");
    let hostport = authority.rsplit('@').next().unwrap_or("");
    let host = match hostport.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => hostport.split(':').next().unwrap_or(""),
    };
    (Some(scheme), (!host.is_empty()).then_some(host))
}

/// The proxy URL rule of the torrent settings screen: a scheme and a host, and the scheme is one of
/// `http, https, socks4, socks4a, socks5, socks5h`.
pub fn validate_proxy_url(s: &str) -> Result<(), ProxyUrlError> {
    match scheme_and_host(s) {
        (Some(scheme), Some(_)) if PROXY_SCHEMES.contains(&scheme) => Ok(()),
        (Some(scheme), Some(_)) => Err(ProxyUrlError::UnsupportedProtocol(scheme.to_string())),
        _ => Err(ProxyUrlError::InvalidUri),
    }
}

/// The reset-trackers action is enabled only while torrents are on and the stored value is not the default.
pub fn is_reset_enabled(torrents_enabled: bool, trackers: &str) -> bool {
    torrents_enabled && trackers != TORRSERVER_TRACKERS
}

impl TorrentPrefs {
    /// Reset default torrent trackers string: deletes the key, so the default shows again.
    pub fn reset_trackers(&self) -> Result<(), crate::PrefError> {
        self.torr_server_trackers.delete()
    }

    /// Call when the enable switch changes. Returns true when the legal notice must be shown now: the first
    /// time torrents are turned on. It records `pref_torrserver_shownotice`, so later enables show nothing.
    pub fn enable_changed(&self, enabled: bool) -> bool {
        if !enabled || self.torr_server_shown_notice.get() {
            return false;
        }
        let _ = self.torr_server_shown_notice.set(true);
        true
    }
}

define_prefs! {
    /// `[torrent]` section: the six keys of Aniyomi `TorrentPreferences`.
    pub struct TorrentPrefs in "torrent" {
        torr_server_enable: bool = "pref_torrserver_enable" => false;
        torr_server_shown_notice: bool = "pref_torrserver_shownotice" => false;
        torr_server_port: String = "pref_torrserver_port" => String::from("8090");
        torr_server_trackers: String = "pref_torrserver_tackers" => String::from(TORRSERVER_TRACKERS);
        torr_server_proxy_mode: ProxyMode = "pref_torrserver_proxymode" => ProxyMode::None;
        torr_server_proxy_url: String = "pref_torrserver_proxyurl" => String::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::PrefStore;
    use crate::testutil::assert_section_registered;
    use crate::values::PrefValue;

    #[test]
    fn keys_and_defaults_match_the_registry() {
        let sec = TorrentPrefs::new(PrefStore::in_memory());
        assert_section_registered(TorrentPrefs::TABLE, TorrentPrefs::FIELDS, &sec.defaults());
        assert!(!sec.torr_server_enable.get());
        assert_eq!(sec.torr_server_port.get(), "8090");
        assert_eq!(sec.torr_server_proxy_mode.get(), ProxyMode::None);
        assert_eq!(sec.torr_server_proxy_url.get(), "");
    }

    #[test]
    fn the_default_tracker_list_is_the_26_plan_lines_without_a_trailing_newline() {
        let sec = TorrentPrefs::new(PrefStore::in_memory());
        let trackers = sec.torr_server_trackers.get();
        assert_eq!(trackers.lines().count(), 26);
        assert!(!trackers.ends_with('\n'));
        assert!(!trackers.contains(' '));
        assert_eq!(trackers.chars().count(), 967);
    }

    #[test]
    fn proxy_mode_wire_values_pin_quirk_q6() {
        assert_eq!(ProxyMode::None.wire_value(), "tracker");
        assert_eq!(ProxyMode::Tracker.wire_value(), "tracker");
        assert_eq!(ProxyMode::Peers.wire_value(), "peers");
        assert_eq!(ProxyMode::Full.wire_value(), "full");
    }

    #[test]
    fn proxy_mode_is_stored_by_its_kotlin_name() {
        let store = PrefStore::in_memory();
        TorrentPrefs::new(store.clone()).torr_server_proxy_mode.set(ProxyMode::Peers).unwrap();
        assert_eq!(store.get_raw("pref_torrserver_proxymode"), Some(PrefValue::Str("Peers".into())));
        store.set_raw("pref_torrserver_proxymode", PrefValue::Str("SIDEWAYS".into()));
        assert_eq!(TorrentPrefs::new(store).torr_server_proxy_mode.get(), ProxyMode::None);
    }

    #[test]
    fn validate_port_vectors() {
        assert_eq!(validate_port(""), Err(InvalidPort));
        assert_eq!(validate_port("0"), Ok(0));
        assert_eq!(validate_port("65535"), Ok(65535));
        assert_eq!(validate_port("65536"), Err(InvalidPort));
        assert_eq!(validate_port("-1"), Err(InvalidPort));
        assert_eq!(validate_port("+80"), Ok(80));
        assert_eq!(validate_port("8o"), Err(InvalidPort));
        assert_eq!(validate_port(" 80"), Err(InvalidPort));
        assert_eq!(validate_port("99999999999"), Err(InvalidPort));
    }

    #[test]
    fn validate_proxy_url_vectors() {
        for ok in ["http://h", "https://h:8", "socks4://h", "socks4a://h", "socks5://h:1080", "socks5h://h", "http://u:p@h:1/x", "http://[::1]:9"] {
            assert_eq!(validate_proxy_url(ok), Ok(()), "{ok}");
        }
        assert_eq!(validate_proxy_url("ftp://h"), Err(ProxyUrlError::UnsupportedProtocol("ftp".into())));
        assert_eq!(validate_proxy_url("http:/h"), Err(ProxyUrlError::InvalidUri));
        assert_eq!(validate_proxy_url(""), Err(ProxyUrlError::InvalidUri));
        assert_eq!(validate_proxy_url("http://"), Err(ProxyUrlError::InvalidUri));
        assert_eq!(validate_proxy_url("h:1080"), Err(ProxyUrlError::InvalidUri));
        assert_eq!(validate_proxy_url("//h"), Err(ProxyUrlError::InvalidUri));
    }

    #[test]
    fn the_server_gets_no_proxy_mode_for_none() {
        assert_eq!(ProxyMode::None.server_mode(), None);
        assert_eq!(ProxyMode::Tracker.server_mode(), Some("tracker"));
        assert_eq!(ProxyMode::Peers.server_mode(), Some("peers"));
        assert_eq!(ProxyMode::Full.server_mode(), Some("full"));
    }

    #[test]
    fn the_reset_action_is_enabled_only_when_on_and_changed() {
        assert!(!is_reset_enabled(false, "custom"));
        assert!(!is_reset_enabled(true, TORRSERVER_TRACKERS));
        assert!(is_reset_enabled(true, "custom"));
    }

    #[test]
    fn reset_deletes_the_trackers_key() {
        let store = PrefStore::in_memory();
        let sec = TorrentPrefs::new(store.clone());
        sec.torr_server_trackers.set("udp://x".into()).unwrap();
        assert!(sec.torr_server_trackers.is_set());
        sec.reset_trackers().unwrap();
        assert_eq!(store.get_raw("pref_torrserver_tackers"), None);
        assert_eq!(sec.torr_server_trackers.get(), TORRSERVER_TRACKERS);
    }

    #[test]
    fn the_notice_shows_on_the_first_enable_only() {
        let store = PrefStore::in_memory();
        let sec = TorrentPrefs::new(store.clone());
        assert!(!sec.enable_changed(false));
        assert!(!sec.torr_server_shown_notice.get());
        assert!(sec.enable_changed(true));
        assert!(sec.torr_server_shown_notice.get());
        assert!(!sec.enable_changed(true));
        assert!(!TorrentPrefs::new(store).enable_changed(true));
    }
}
