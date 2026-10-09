//! Video URL resolution (area 45 `PlayerUtils.kt` `resolveUri`, `PlayerActivity.parseVideoUrl`,
//! T-45a-010). Pure string handling: `file:` becomes a path, `data:` becomes `data://<payload>`,
//! protocols that mpv reads pass through, and anything else is rejected.

/// Network schemes mpv opens directly. Kotlin read `Utils.PROTOCOLS` from the mpv-android
/// library; this list is the mpv manual's network set and should be checked on libmpv upgrades.
pub const PROTOCOLS: &[&str] = &[
    "http", "https", "ftp", "ftps", "rtmp", "rtmps", "rtmpe", "rtmpt", "rtmpts", "rtsp", "rtp",
    "udp", "tcp", "srt", "mms", "mmsh", "smb", "sftp",
];

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    #[error("unknown scheme: {0}")]
    UnknownScheme(String),
}

/// `Uri.resolveUri` for the Linux case. `content:` has no path on desktop and is rejected.
pub fn resolve_uri(url: &str) -> Result<String, SourceError> {
    let Some((scheme, rest)) = split_scheme(url) else {
        return Err(SourceError::UnknownScheme(String::new()));
    };
    match scheme.to_ascii_lowercase().as_str() {
        "file" => Ok(file_path(rest)),
        "data" => Ok(format!("data://{rest}")),
        other if PROTOCOLS.contains(&other) => Ok(url.to_owned()),
        _ => Err(SourceError::UnknownScheme(scheme.to_owned())),
    }
}

/// `PlayerActivity.parseVideoUrl`: `None` in gives `None` out. A URL that does not resolve is
/// passed through unchanged, after the error is logged (`?: videoUrl` in Kotlin).
pub fn parse_video_url(url: Option<&str>) -> Option<String> {
    let url = url?;
    Some(resolve_uri(url).unwrap_or_else(|e| {
        tracing::error!("{e}");
        url.to_owned()
    }))
}

/// Splits `scheme:rest` when the scheme is syntactically valid (RFC 3986).
fn split_scheme(url: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = url.split_once(':')?;
    let mut chars = scheme.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let valid =
        first_ok && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then_some((scheme, rest))
}

/// Path of a `file:` URI: drops an authority (`file://host/x` gives `/x`), the query and the
/// fragment, then percent-decodes, as `Uri.path` does.
fn file_path(rest: &str) -> String {
    let without_suffix = rest.split(['?', '#']).next().unwrap_or("");
    let path = match without_suffix.strip_prefix("//") {
        Some(after_slashes) => {
            &after_slashes[after_slashes.find('/').unwrap_or(after_slashes.len())..]
        }
        None => without_suffix,
    };
    percent_decode(path)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| std::str::from_utf8(&bytes[i + 1..i + 3]).ok())
            .flatten()
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_scheme_gives_path() {
        assert_eq!(
            resolve_uri("file:///home/a/b.mp4").unwrap(),
            "/home/a/b.mp4"
        );
        assert_eq!(resolve_uri("file://host/x.mkv").unwrap(), "/x.mkv");
        assert_eq!(resolve_uri("file:///a/b.mp4?x=1#frag").unwrap(), "/a/b.mp4");
    }

    #[test]
    fn spaces_preserved_and_percent_decoded() {
        assert_eq!(resolve_uri("file:///a b/c.mp4").unwrap(), "/a b/c.mp4");
        assert_eq!(resolve_uri("file:///a%20b/c.mp4").unwrap(), "/a b/c.mp4");
    }

    #[test]
    fn data_scheme_keeps_payload() {
        assert_eq!(
            resolve_uri("data:video/mp4;base64,AAAA,BBB").unwrap(),
            "data://video/mp4;base64,AAAA,BBB"
        );
    }

    #[test]
    fn protocol_passthrough() {
        for url in [
            "http://a.test/v.m3u8",
            "https://a.test/v",
            "rtmp://h/app",
            "udp://239.0.0.1:1234",
        ] {
            assert_eq!(resolve_uri(url).unwrap(), url);
        }
    }

    #[test]
    fn content_scheme_rejected() {
        let err = resolve_uri("content://media/external/video/1").unwrap_err();
        assert_eq!(err.to_string(), "unknown scheme: content");
    }

    #[test]
    fn unknown_scheme_is_none_and_parse_falls_back_to_input() {
        assert!(resolve_uri("gopher://x").is_err());
        assert_eq!(
            parse_video_url(Some("gopher://x")).as_deref(),
            Some("gopher://x")
        );
    }

    #[test]
    fn null_input_gives_none() {
        assert_eq!(parse_video_url(None), None);
        assert_eq!(
            parse_video_url(Some("file:///a.mp4")).as_deref(),
            Some("/a.mp4")
        );
    }
}
