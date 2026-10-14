//! A local HLS relay for the player. mpv's network layer (ffmpeg) speaks HTTP/1.1 only and has its own TLS, and some
//! video CDNs refuse that (AnimePahe's answers 403 to any HTTP/1.1 request). The app's HTTP client does what a browser
//! does, so a remote `.m3u8` stream is played through this relay on 127.0.0.1: mpv asks it, it fetches from the CDN with
//! the video's headers, and it rewrites each playlist so the segments, keys and sub-playlists come through it too.
//! Aniyomi plays every stream through its own client the same way.
//!
//! Each upstream address travels in the local path as hex, ending in an extension ffmpeg accepts for its role
//! (`.m3u8`, `.ts`, `.key`, `.mp4`): some CDNs disguise segments as `.jpg`, which ffmpeg refuses.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

/// The user agent sent when the video names none: a desktop browser's.
const BROWSER_UA: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";

/// How many of the latest played videos keep their headers in the relay.
const KEEP_VIDEOS: u64 = 16;

struct Relay {
    port: u16,
    /// The header set of each played video, by the token in its local paths.
    headers: Mutex<HashMap<u64, Vec<(String, String)>>>,
    next: Mutex<u64>,
    client: reqwest::blocking::Client,
}

fn relay() -> Option<&'static Arc<Relay>> {
    static RELAY: OnceLock<Option<Arc<Relay>>> = OnceLock::new();
    RELAY
        .get_or_init(|| {
            let listener = TcpListener::bind("127.0.0.1:0").inspect_err(|e| tracing::warn!(%e, "the HLS relay could not start")).ok()?;
            let client = reqwest::blocking::Client::builder().build().inspect_err(|e| tracing::warn!(%e, "the HLS relay has no client")).ok()?;
            let relay = Arc::new(Relay { port: listener.local_addr().ok()?.port(), headers: Mutex::default(), next: Mutex::new(0), client });
            let serving = Arc::clone(&relay);
            std::thread::Builder::new()
                .name("kuro-hls-relay".into())
                .spawn(move || {
                    for conn in listener.incoming().flatten() {
                        let relay = Arc::clone(&serving);
                        let _ = std::thread::Builder::new().name("kuro-hls-conn".into()).spawn(move || relay.serve(conn));
                    }
                })
                .ok()?;
            Some(relay)
        })
        .as_ref()
}

/// Whether `url` is a remote HLS playlist, the kind the relay plays.
pub fn wants_relay(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://")) && url.split(['?', '#']).next().is_some_and(|path| path.ends_with(".m3u8"))
}

/// The local address that plays `url` with `headers`, or `url` itself when the relay cannot start.
pub fn local_url(url: &str, headers: &[(String, String)]) -> String {
    let Some(relay) = relay() else { return url.to_owned() };
    let token = {
        let mut next = relay.next.lock().unwrap_or_else(PoisonError::into_inner);
        *next += 1;
        *next
    };
    let mut sets = relay.headers.lock().unwrap_or_else(PoisonError::into_inner);
    sets.insert(token, headers.to_vec());
    // only the videos played lately can still be asked for
    sets.retain(|kept, _| *kept + KEEP_VIDEOS > token);
    drop(sets);
    local(relay.port, token, url, "m3u8")
}

fn local(port: u16, token: u64, upstream: &str, extension: &str) -> String {
    format!("http://127.0.0.1:{port}/{token}/{}.{extension}", hex(upstream.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<String> {
    let bytes: Option<Vec<u8>> = (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect();
    String::from_utf8(bytes?).ok()
}

/// `(token, upstream)` of a local path `/<token>/<hex>.<ext>`.
fn parse_path(path: &str) -> Option<(u64, String)> {
    let (token, rest) = path.strip_prefix('/')?.split_once('/')?;
    let hex = rest.rsplit_once('.').map_or(rest, |(h, _)| h);
    Some((token.parse().ok()?, unhex(hex)?))
}

/// The upstream address a local relay address stands for.
#[cfg(test)]
pub fn upstream_of(local_url: &str) -> Option<String> {
    let path = local_url.strip_prefix("http://127.0.0.1:")?.split_once('/')?.1;
    parse_path(&format!("/{path}")).map(|(_, upstream)| upstream)
}

/// Rewrites a playlist so every address in it (lines and `URI="..."` attributes) points at the relay. Relative addresses
/// are resolved against `base`, the playlist's own address.
fn rewrite(playlist: &str, base: &url::Url, port: u16, token: u64) -> String {
    let resolve = |uri: &str| base.join(uri).map(|u| u.to_string()).unwrap_or_else(|_| uri.to_owned());
    let mut out = String::with_capacity(playlist.len() * 2);
    // a line after #EXT-X-STREAM-INF (a master playlist's variant) is a playlist; any other address line is a segment
    let mut next_is_playlist = false;
    for line in playlist.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        if trimmed.starts_with('#') {
            next_is_playlist |= trimmed.starts_with("#EXT-X-STREAM-INF");
            let extension = if trimmed.starts_with("#EXT-X-KEY") || trimmed.starts_with("#EXT-X-SESSION-KEY") {
                "key"
            } else if trimmed.starts_with("#EXT-X-MAP") {
                "mp4"
            } else {
                "m3u8"
            };
            out.push_str(&rewrite_uri_attribute(trimmed, |uri| local(port, token, &resolve(uri), extension)));
        } else {
            let extension = if std::mem::take(&mut next_is_playlist) { "m3u8" } else { "ts" };
            out.push_str(&local(port, token, &resolve(trimmed), extension));
        }
        out.push('\n');
    }
    out
}

/// The tag line with its `URI="..."` value replaced by `map(value)`.
fn rewrite_uri_attribute(line: &str, map: impl Fn(&str) -> String) -> String {
    let Some(start) = line.find("URI=\"") else { return line.to_owned() };
    let value_start = start + 5;
    let Some(len) = line[value_start..].find('"') else { return line.to_owned() };
    format!("{}{}{}", &line[..value_start], map(&line[value_start..value_start + len]), &line[value_start + len..])
}

impl Relay {
    fn serve(&self, conn: TcpStream) {
        let mut reader = BufReader::new(match conn.try_clone() {
            Ok(c) => c,
            Err(_) => return,
        });
        let mut writer = conn;
        // one request per connection: ffmpeg opens a new one for each segment, and the answer closes it
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            return;
        }
        let mut range = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.trim().eq_ignore_ascii_case("range") {
                    range = Some(value.trim().to_owned());
                }
            }
        }
        let path = request_line.split_whitespace().nth(1).unwrap_or_default();
        if let Err(error) = self.answer(&mut writer, path, range.as_deref()) {
            tracing::debug!(%error, "the HLS relay could not answer");
            let _ = writer.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    }

    fn answer(&self, writer: &mut TcpStream, path: &str, range: Option<&str>) -> Result<(), String> {
        let (token, upstream) = parse_path(path).ok_or("not a relay path")?;
        let headers = self.headers.lock().unwrap_or_else(PoisonError::into_inner).get(&token).cloned().ok_or("unknown video")?;
        let mut request = self.client.get(&upstream);
        if !headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("user-agent")) {
            request = request.header("User-Agent", BROWSER_UA);
        }
        for (name, value) in &headers {
            request = request.header(name, value);
        }
        if let Some(range) = range {
            request = request.header("Range", range);
        }
        let mut response = request.send().map_err(|e| e.to_string())?;
        let status = response.status();
        let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or_default().to_owned();
        if path.ends_with(".m3u8") || content_type.contains("mpegurl") {
            let base = response.url().clone();
            let text = response.text().map_err(|e| e.to_string())?;
            let body = if text.trim_start().starts_with("#EXTM3U") { rewrite(&text, &base, self.port, token) } else { text };
            let head = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                status.as_u16(),
                status.canonical_reason().unwrap_or(""),
                body.len()
            );
            writer.write_all(head.as_bytes()).and_then(|()| writer.write_all(body.as_bytes())).map_err(|e| e.to_string())?;
            return Ok(());
        }
        let mut head = format!("HTTP/1.1 {} {}\r\nConnection: close\r\n", status.as_u16(), status.canonical_reason().unwrap_or(""));
        for name in ["content-length", "content-range", "accept-ranges"] {
            if let Some(value) = response.headers().get(name).and_then(|v| v.to_str().ok()) {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
        }
        // the CDN's own type may be image/jpeg for a disguised segment: ffmpeg probes the bytes, so none is sent
        head.push_str("\r\n");
        writer.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = response.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                return Ok(());
            }
            writer.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_remote_playlists_are_relayed() {
        assert!(wants_relay("https://cdn.test/a/uwu.m3u8"));
        assert!(wants_relay("https://cdn.test/a/index.m3u8?token=1"));
        assert!(!wants_relay("https://cdn.test/a/video.mp4"));
        assert!(!wants_relay("/home/me/video.m3u8"));
    }

    #[test]
    fn a_local_path_carries_its_token_and_upstream() {
        let path = local(1, 7, "https://cdn.test/s/segment-1.jpg?x=1", "ts");
        let path = path.strip_prefix("http://127.0.0.1:1").unwrap();
        assert!(path.ends_with(".ts"));
        assert_eq!(parse_path(path), Some((7, "https://cdn.test/s/segment-1.jpg?x=1".to_owned())));
    }

    #[test]
    fn a_playlist_points_its_segments_key_and_variants_at_the_relay() {
        let base = url::Url::parse("https://cdn.test/v/uwu.m3u8").unwrap();
        let playlist = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"mon.key\"\n#EXTINF:9.9,\nsegment-1.jpg\n#EXT-X-STREAM-INF:BANDWIDTH=1\nhd/index.m3u8\n";
        let out = rewrite(playlist, &base, 9, 3);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "#EXTM3U");
        assert!(lines[1].starts_with("#EXT-X-KEY:METHOD=AES-128,URI=\"http://127.0.0.1:9/3/") && lines[1].ends_with(".key\""));
        assert!(lines[3].ends_with(".ts"));
        assert_eq!(parse_path(lines[3].strip_prefix("http://127.0.0.1:9").unwrap()).unwrap().1, "https://cdn.test/v/segment-1.jpg");
        assert!(lines[5].ends_with(".m3u8"), "a variant stays a playlist");
        assert_eq!(parse_path(lines[5].strip_prefix("http://127.0.0.1:9").unwrap()).unwrap().1, "https://cdn.test/v/hd/index.m3u8");
    }
}
