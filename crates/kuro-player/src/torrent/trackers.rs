//! The default BitTorrent trackers of the torrent server (data doc section 6.1, `TorrentPreferences.kt:11-34`).
//! The app never contacts these hosts; TorrServer announces to them.

/// The default value of `pref_torrserver_tackers`: the 26 URLs, one per line.
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
