//! Redaction for log lines (T-24a-060; edge cases section 6). A URL keeps its scheme, host and path; its userinfo, query
//! string and fragment are removed, because signed links and tokens live there. Header values are never logged: each
//! pair prints as its name and `<redacted>`.

/// The URL without `user:pass@`, query string and fragment. A URL with no `://` (a magnet link, for one) keeps only its
/// scheme.
pub fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        let scheme = url.split_once(':').map_or("", |(scheme, _)| scheme);
        return format!("{scheme}:<redacted>");
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest[..authority_end].rsplit('@').next().unwrap_or_default();
    let path_end = rest[authority_end..].find(['?', '#']).map_or(rest.len(), |i| authority_end + i);
    format!("{scheme}://{authority}{}", &rest[authority_end..path_end])
}

/// A log text with every URL or magnet link in it redacted. Words are split on spaces and joined back the same way.
pub fn redact_line(line: &str) -> String {
    line.split(' ')
        .map(|word| if word.contains("://") || word.starts_with("magnet:") { redact_url(word) } else { word.to_owned() })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Header pairs for a log: `Name: <redacted>`, joined by `, `. The values are never shown.
pub fn redacted_headers(headers: &[(String, String)]) -> String {
    headers.iter().map(|(name, _)| format!("{name}: <redacted>")).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_fragment_and_userinfo_are_removed() {
        assert_eq!(redact_url("https://user:pw@cdn.example.org/v/ep1.mkv?token=abc#x"), "https://cdn.example.org/v/ep1.mkv");
        assert_eq!(redact_url("http://host?sig=1"), "http://host");
        assert_eq!(redact_url("https://host/only/path"), "https://host/only/path");
    }

    #[test]
    fn a_magnet_link_keeps_only_its_scheme() {
        assert_eq!(redact_url("magnet:?xt=urn:btih:abc&index=2"), "magnet:<redacted>");
    }

    #[test]
    fn redact_line_redacts_each_url_and_keeps_the_rest() {
        assert_eq!(
            redact_line("Error opening https://h.org/a.m3u8?key=secret for reading"),
            "Error opening https://h.org/a.m3u8 for reading"
        );
        assert_eq!(redact_line("Connection refused"), "Connection refused");
    }

    #[test]
    fn header_values_never_reach_the_log() {
        let headers = vec![("Cookie".to_owned(), "session=secret".to_owned()), ("Referer".to_owned(), "https://x".to_owned())];
        let text = redacted_headers(&headers);
        assert_eq!(text, "Cookie: <redacted>, Referer: <redacted>");
        assert!(!text.contains("secret"));
    }
}
