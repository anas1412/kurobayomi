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
}
