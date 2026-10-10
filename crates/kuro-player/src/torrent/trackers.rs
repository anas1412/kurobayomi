//! The default BitTorrent trackers of the torrent server (T-45a-076; data doc section 6.1, `TorrentPreferences.kt:11-34`).
//! The app never contacts these hosts; TorrServer announces to them.

use super::links::url_encode;

/// The 26 default announce URLs, in the source order.
pub const DEFAULT_TRACKERS: [&str; 26] = [
    "http://nyaa.tracker.wf:7777/announce",
    "http://anidex.moe:6969/announce",
    "http://tracker.anirena.com:80/announce",
    "udp://tracker.uw0.xyz:6969/announce",
    "http://share.camoe.cn:8080/announce",
    "http://t.nyaatracker.com:80/announce",
    "udp://47.ip-51-68-199.eu:6969/announce",
    "udp://9.rarbg.me:2940",
    "udp://9.rarbg.to:2820",
    "udp://exodus.desync.com:6969/announce",
    "udp://explodie.org:6969/announce",
    "udp://ipv4.tracker.harry.lu:80/announce",
    "udp://open.stealth.si:80/announce",
    "udp://opentor.org:2710/announce",
    "udp://opentracker.i2p.rocks:6969/announce",
    "udp://retracker.lanta-net.ru:2710/announce",
    "udp://tracker.cyberia.is:6969/announce",
    "udp://tracker.dler.org:6969/announce",
    "udp://tracker.ds.is:6969/announce",
    "udp://tracker.internetwarriors.net:1337",
    "udp://tracker.openbittorrent.com:6969/announce",
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://tracker.tiny-vps.com:6969/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://valakas.rollo.dnsabr.com:2710/announce",
    "udp://www.torrent.eu.org:451/announce",
];

/// The default value of `pref_torrserver_tackers`: the 26 URLs, one per line (`DEFAULT_TRACKERS` joined with newlines).
pub const DEFAULT_TRACKER_PREF: &str = concat!(
    "http://nyaa.tracker.wf:7777/announce\n",
    "http://anidex.moe:6969/announce\n",
    "http://tracker.anirena.com:80/announce\n",
    "udp://tracker.uw0.xyz:6969/announce\n",
    "http://share.camoe.cn:8080/announce\n",
    "http://t.nyaatracker.com:80/announce\n",
    "udp://47.ip-51-68-199.eu:6969/announce\n",
    "udp://9.rarbg.me:2940\n",
    "udp://9.rarbg.to:2820\n",
    "udp://exodus.desync.com:6969/announce\n",
    "udp://explodie.org:6969/announce\n",
    "udp://ipv4.tracker.harry.lu:80/announce\n",
    "udp://open.stealth.si:80/announce\n",
    "udp://opentor.org:2710/announce\n",
    "udp://opentracker.i2p.rocks:6969/announce\n",
    "udp://retracker.lanta-net.ru:2710/announce\n",
    "udp://tracker.cyberia.is:6969/announce\n",
    "udp://tracker.dler.org:6969/announce\n",
    "udp://tracker.ds.is:6969/announce\n",
    "udp://tracker.internetwarriors.net:1337\n",
    "udp://tracker.openbittorrent.com:6969/announce\n",
    "udp://tracker.opentrackr.org:1337/announce\n",
    "udp://tracker.tiny-vps.com:6969/announce\n",
    "udp://tracker.torrent.eu.org:451/announce\n",
    "udp://valakas.rollo.dnsabr.com:2710/announce\n",
    "udp://www.torrent.eu.org:451/announce",
);

/// The trackers of the stored preference, one per non-empty trimmed line.
fn lines(stored: &str) -> impl Iterator<Item = &str> {
    stored
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
}

/// The list as the server takes it: the stored text split on newlines and joined with `,\n`
/// (`TorrentServerUtils.kt:11-19`). Blank lines are dropped, so a trailing newline adds no empty tracker.
pub fn tracker_list_for_server(stored: &str) -> String {
    lines(stored).collect::<Vec<_>>().join(",\n")
}

/// A magnet link with every tracker of the stored list added as `&tr=<url-encoded>` (area 13 section 5, option b).
pub fn with_trackers(magnet: &str, stored: &str) -> String {
    lines(stored).fold(magnet.to_owned(), |mut link, tracker| {
        link.push_str("&tr=");
        link.push_str(&url_encode(tracker));
        link
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_list_has_26_trackers_in_source_order() {
        assert_eq!(DEFAULT_TRACKERS.len(), 26);
        assert_eq!(DEFAULT_TRACKERS[0], "http://nyaa.tracker.wf:7777/announce");
        assert_eq!(
            DEFAULT_TRACKERS[25],
            "udp://www.torrent.eu.org:451/announce"
        );
        assert!(DEFAULT_TRACKERS.iter().all(|t| !t.contains(' ')));
        assert_eq!(DEFAULT_TRACKER_PREF.lines().count(), 26);
        assert_eq!(DEFAULT_TRACKER_PREF, DEFAULT_TRACKERS.join("\n"));
    }

    #[test]
    fn the_stored_list_is_split_and_joined_with_comma_newline() {
        assert_eq!(
            tracker_list_for_server("udp://a:1/announce\n udp://b:2 \n"),
            "udp://a:1/announce,\nudp://b:2"
        );
        assert_eq!(tracker_list_for_server(""), "");
    }

    #[test]
    fn magnets_get_every_tracker_url_encoded() {
        assert_eq!(
            with_trackers("magnet:?xt=urn:btih:abc", "http://t.io/a b\nudp://u:1"),
            "magnet:?xt=urn:btih:abc&tr=http%3A%2F%2Ft.io%2Fa+b&tr=udp%3A%2F%2Fu%3A1"
        );
    }
}
