//! The two fixture cases of `TorrentHelpersTest.kt` (T-13a-058). The fixtures are byte copies of the
//! Aniyomi test resources; their SHA-256 is pinned so a changed fixture fails here first.

use kuro_core::hash::sha256_hex;
use kuro_core::torrent::meta::{FileStat, parse_torrent_file};

const FILE: &[u8] = include_bytes!("fixtures/torrent/test-torrent-file.torrent");
const DIR: &[u8] = include_bytes!("fixtures/torrent/test-torrent-directory.torrent");

const FILE_SHA256: &str = "786de5e7aa30cbdd6d0b0d7b21afc4eac7348eeb2a2cf06c1a1965cdd283ef6b";
const DIR_SHA256: &str = "4d00df7310fd2a9eabe8ea4c8169c8ff6697e09f88a031912587516a3114f09c";

#[test]
fn fixtures_are_the_recorded_bytes() {
    assert_eq!(sha256_hex(FILE), FILE_SHA256);
    assert_eq!(sha256_hex(DIR), DIR_SHA256);
}

#[test]
fn single_file_torrent() {
    let m = parse_torrent_file(FILE).unwrap();
    assert_eq!(m.title, "test-torrent-file");
    assert_eq!(m.hash, "104701de4b710ee9720ee249bf220384659e8c38");
    assert_eq!(m.size, 18);
    assert_eq!(m.trackers, ["udp://tracker.opentrackr.org:1337/announce"]);
    assert_eq!(m.files, [FileStat { id: None, path: "test-torrent-file".into(), length: 18 }]);
}

#[test]
fn directory_torrent() {
    let m = parse_torrent_file(DIR).unwrap();
    assert_eq!(m.title, "test-torrent-directory");
    assert_eq!(m.hash, "863eb76f1cd0643d9fd2eb4a7da866c3752991b5");
    assert_eq!(m.size, 12);
    assert_eq!(
        m.trackers,
        [
            "udp://tracker.opentrackr.org:1337/announce",
            "udp://tracker.opentrackr.org:1337/announce",
            "https://tracker.tamersunion.org:443/announce",
            "http://tracker.ipv6tracker.org:80/announce",
            "udp://opentracker.io:6969/announce",
            "udp://tracker.torrent.eu.org:451/announce",
        ]
    );
    let files: Vec<_> = m.files.iter().map(|f| (f.id, f.path.as_str(), f.length)).collect();
    assert_eq!(files, [(Some(1), "3/3.1", 4), (Some(2), "2", 2), (Some(3), "1", 2), (Some(4), "3/3.2", 4)]);
}
