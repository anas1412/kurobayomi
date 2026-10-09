//! Torrent metadata from a `.torrent` file (port of `TorrentHelpers.parseTorrentDetailsFromTorrentFileContent`).

use super::TorrentError;
use super::bencode::{self, Value};
use crate::util::hash::sha1_hex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    /// 1-based for multi-file torrents, `None` for a single-file torrent.
    pub id: Option<u32>,
    pub path: String,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentMeta {
    pub title: String,
    /// 40 lowercase hex characters: SHA-1 of the re-encoded `info` dictionary.
    pub hash: String,
    pub size: u64,
    /// `announce` first, then `announce-list` flattened; duplicates and empty strings are kept.
    pub trackers: Vec<String>,
    pub files: Vec<FileStat>,
}

/// One playable file of a torrent (`TorrentFile.kt`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentFile {
    pub path: String,
    pub index: i32,
    pub size: i64,
    pub hash: String,
    pub trackers: Vec<String>,
}

/// What a source gets back for a magnet link or `.torrent` URL (`TorrentInfo.kt`). `size` is -1 when unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentInfo {
    pub title: String,
    pub files: Vec<TorrentFile>,
    pub hash: String,
    pub size: i64,
    pub trackers: Vec<String>,
}

fn invalid(why: &str) -> TorrentError {
    TorrentError::InvalidTorrent(why.to_string())
}

fn text(v: &Value) -> Result<String, TorrentError> {
    match v {
        Value::Bytes(b) => String::from_utf8(b.clone()).map_err(|_| invalid("text is not UTF-8")),
        _ => Err(invalid("expected a string")),
    }
}

fn length(v: Option<&Value>) -> Result<u64, TorrentError> {
    match v {
        Some(Value::Int(n)) => u64::try_from(*n).map_err(|_| invalid("negative length")),
        _ => Err(invalid("expected an integer length")),
    }
}

pub fn parse_torrent_file(bytes: &[u8]) -> Result<TorrentMeta, TorrentError> {
    let root = bencode::parse(bytes).map_err(|e| invalid(&e.to_string()))?;
    if !matches!(root, Value::Dict(_)) {
        return Err(invalid("not a dictionary"));
    }
    let info = root.get("info").filter(|i| matches!(i, Value::Dict(_))).ok_or_else(|| invalid("missing info"))?;
    let title = text(info.get("name").ok_or_else(|| invalid("missing name"))?)?;
    let hash = sha1_hex(&bencode::encode(info)?);

    let mut trackers = Vec::new();
    if let Some(a) = root.get("announce") {
        trackers.push(text(a)?);
    }
    if let Some(list) = root.get("announce-list") {
        let Value::List(tiers) = list else { return Err(invalid("announce-list is not a list")) };
        for tier in tiers {
            let Value::List(urls) = tier else { return Err(invalid("announce tier is not a list")) };
            for url in urls {
                trackers.push(text(url)?);
            }
        }
    }

    let (size, files) = match (info.get("length"), info.get("files")) {
        (Some(_), None) => {
            let len = length(info.get("length"))?;
            (len, vec![FileStat { id: None, path: title.clone(), length: len }])
        }
        (None, Some(Value::List(entries))) => {
            let mut total = 0u64;
            let mut files = Vec::with_capacity(entries.len());
            for (i, entry) in entries.iter().enumerate() {
                if !matches!(entry, Value::Dict(_)) {
                    return Err(invalid("file entry is not a dictionary"));
                }
                let len = length(entry.get("length"))?;
                let Some(Value::List(parts)) = entry.get("path") else { return Err(invalid("file path is not a list")) };
                let path = parts.iter().map(text).collect::<Result<Vec<_>, _>>()?.join("/");
                total = total.checked_add(len).ok_or_else(|| invalid("size overflow"))?;
                files.push(FileStat { id: Some(i as u32 + 1), path, length: len });
            }
            (total, files)
        }
        _ => return Err(invalid("Invalid torrent file")),
    };
    Ok(TorrentMeta { title, hash, size, trackers, files })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &[u8] = include_bytes!("../../tests/fixtures/torrent/test-torrent-file.torrent");
    const DIR: &[u8] = include_bytes!("../../tests/fixtures/torrent/test-torrent-directory.torrent");

    fn s(v: &str) -> Value {
        Value::Bytes(v.as_bytes().to_vec())
    }

    fn dict(pairs: Vec<(&str, Value)>) -> Value {
        Value::Dict(pairs.into_iter().map(|(k, v)| (k.as_bytes().to_vec(), v)).collect())
    }

    fn enc(v: Value) -> Vec<u8> {
        bencode::encode(&v).unwrap()
    }

    #[test]
    fn the_file_fixture() {
        let m = parse_torrent_file(FILE).unwrap();
        assert_eq!(m.title, "test-torrent-file");
        assert_eq!(m.hash, "104701de4b710ee9720ee249bf220384659e8c38");
        assert_eq!(m.size, 18);
        assert_eq!(m.trackers, ["udp://tracker.opentrackr.org:1337/announce"]);
        assert_eq!(m.files, [FileStat { id: None, path: "test-torrent-file".into(), length: 18 }]);
    }

    #[test]
    fn the_directory_fixture() {
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

    #[test]
    fn encode_of_parse_is_identity_and_the_info_hash_matches_the_raw_slice() {
        use crate::util::hash::sha1_hex;
        for (raw, hash) in [(FILE, "104701de4b710ee9720ee249bf220384659e8c38"), (DIR, "863eb76f1cd0643d9fd2eb4a7da866c3752991b5")] {
            let v = bencode::parse(raw).unwrap();
            assert_eq!(bencode::encode(&v).unwrap(), raw);
            let info = bencode::encode(v.get("info").unwrap()).unwrap();
            assert_eq!(sha1_hex(&info), hash);
            // the raw slice of the info dictionary: from "4:info" to the closing 'e' of the outer dictionary
            let at = raw.windows(6).position(|w| w == b"4:info").unwrap() + 6;
            assert_eq!(sha1_hex(&raw[at..raw.len() - 1]), hash);
        }
    }

    #[test]
    fn section_11_2_structural_errors_are_invalid_torrent() {
        let name = || ("name", s("n"));
        let cases = [
            enc(dict(vec![])),
            enc(dict(vec![("info", dict(vec![("length", Value::Int(1))]))])),
            enc(dict(vec![("info", dict(vec![name(), ("length", Value::Int(1)), ("files", Value::List(vec![]))]))])),
            enc(dict(vec![("info", dict(vec![name()]))])),
            enc(dict(vec![("announce", Value::Int(1)), ("info", dict(vec![name(), ("length", Value::Int(1))]))])),
            enc(dict(vec![("info", dict(vec![("name", Value::Bytes(vec![0xff, 0xfe])), ("length", Value::Int(1))]))])),
            enc(dict(vec![("announce-list", Value::List(vec![s("x")])), ("info", dict(vec![name(), ("length", Value::Int(1))]))])),
            b"i1e".to_vec(),
            b"garbage".to_vec(),
        ];
        for c in cases {
            assert!(matches!(parse_torrent_file(&c), Err(TorrentError::InvalidTorrent(_))), "{:?}", String::from_utf8_lossy(&c));
        }
    }

    #[test]
    fn an_empty_path_list_is_an_empty_path_and_empty_trackers_are_kept() {
        let file = dict(vec![("length", Value::Int(3)), ("path", Value::List(vec![]))]);
        let torrent = dict(vec![
            ("announce", s("")),
            ("info", dict(vec![("name", s("n")), ("files", Value::List(vec![file]))])),
        ]);
        let m = parse_torrent_file(&enc(torrent)).unwrap();
        assert_eq!(m.files, [FileStat { id: Some(1), path: String::new(), length: 3 }]);
        assert_eq!(m.trackers, [""]);
    }
}
