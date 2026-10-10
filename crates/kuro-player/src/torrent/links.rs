//! URL encoding and play links of the torrent server (T-45a-075; `TorrentServerUtils.kt:21-31`).

use super::api::{FileStat, Torrent};

/// Java's `URLEncoder.encode(text, "UTF-8")`: ASCII letters, digits and `.-*_` stay, a space becomes `+`, and every
/// other byte of the UTF-8 text becomes `%XX`.
pub fn url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'*' | b'_' => {
                out.push(char::from(byte));
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// The basename of a path (the part after the last `/`).
fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The play link of file `index` of a torrent:
/// `http://127.0.0.1:<port>/stream/<name>?link=<hash>&index=<index>&play`. The name is the basename of the file whose
/// id is `index`, or the torrent title when no file has that id.
pub fn play_link(port: u16, torrent: &Torrent, index: i64) -> String {
    let files: &[FileStat] = torrent.file_stats.as_deref().unwrap_or(&[]);
    let name = files
        .iter()
        .find(|file| file.id == Some(index))
        .map_or(torrent.title.as_str(), |file| basename(&file.path));
    let hash = torrent.hash.as_deref().unwrap_or("");
    format!(
        "http://127.0.0.1:{port}/stream/{}?link={hash}&index={index}&play",
        url_encode(name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn torrent() -> Torrent {
        Torrent {
            title: "Show Pack".into(),
            hash: Some("abc123".into()),
            torrent_size: None,
            trackers: None,
            file_stats: Some(vec![
                FileStat {
                    id: Some(1),
                    path: "Show/Episode 01.mkv".into(),
                    length: 1,
                },
                FileStat {
                    id: Some(2),
                    path: "Show/Épisode 02 & more.mkv".into(),
                    length: 1,
                },
            ]),
        }
    }

    #[test]
    fn encoding_matches_the_java_url_encoder() {
        assert_eq!(url_encode("a b"), "a+b");
        assert_eq!(url_encode("a.b-c*d_e~f"), "a.b-c*d_e%7Ef");
        assert_eq!(url_encode("é&"), "%C3%A9%26");
    }

    #[test]
    fn the_play_link_uses_the_basename_of_the_file_with_that_id() {
        assert_eq!(
            play_link(8090, &torrent(), 1),
            "http://127.0.0.1:8090/stream/Episode+01.mkv?link=abc123&index=1&play"
        );
    }

    #[test]
    fn the_title_names_the_link_when_no_file_has_the_index() {
        assert_eq!(
            play_link(8090, &torrent(), 9),
            "http://127.0.0.1:8090/stream/Show+Pack?link=abc123&index=9&play"
        );
    }
}
