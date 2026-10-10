//! The release notes of the update page (T-47ba-031): markdown to Pango markup, and the link-button rule.
//!
//! Decision: a small converter instead of `pulldown-cmark`. Release notes use headings, bullets, bold, inline code and
//! links; those are what this renders. Everything else is shown as escaped text. Pango's own link colour follows the theme.

/// The Pango markup of `markdown`: headings in bold at three sizes, `-`, `*` and `+` bullets as `•`, `**bold**`,
/// `` `code` `` and `[text](url)` links. A horizontal rule line is dropped. Markup characters in the text are escaped.
pub fn to_pango(markdown: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in markdown.lines() {
        let line = line.trim_end();
        let trimmed = line.trim_start();
        if matches!(trimmed, "---" | "***" | "___") {
            continue;
        }
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            let size = match hashes {
                1 => "xx-large",
                2 => "x-large",
                _ => "large",
            };
            out.push(format!("<span size=\"{size}\" weight=\"bold\">{}</span>", inline(trimmed[hashes..].trim())));
        } else if let Some(item) = ["- ", "* ", "+ "].iter().find_map(|m| trimmed.strip_prefix(m)) {
            let indent = &line[..line.len() - trimmed.len()];
            out.push(format!("{indent}• {}", inline(item.trim())));
        } else {
            out.push(inline(line));
        }
    }
    out.join("\n")
}

/// Whether the release-page button is shown: not when the release link is empty or blank (T-E65).
pub fn link_visible(release_link: &str) -> bool {
    !release_link.trim().is_empty()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Escapes `text` and converts bold, code and links.
fn inline(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some((open, close, tag)) = [("**", "**", "b"), ("`", "`", "tt")].iter().find(|(o, _, _)| rest.starts_with(o)).copied() {
            if let Some(end) = rest[open.len()..].find(close).filter(|e| *e > 0) {
                let body = &rest[open.len()..open.len() + end];
                out.push_str(&format!("<{tag}>{}</{tag}>", escape(body)));
                rest = &rest[open.len() + end + close.len()..];
                continue;
            }
        }
        if rest.starts_with('[') {
            if let Some((label, url, used)) = link(rest) {
                out.push_str(&format!("<a href=\"{}\">{}</a>", escape(url).replace('"', "&quot;"), escape(label)));
                rest = &rest[used..];
                continue;
            }
        }
        let ch = rest.chars().next().expect("not empty");
        out.push_str(&escape(&ch.to_string()));
        rest = &rest[ch.len_utf8()..];
    }
    out
}

/// `[label](url)` at the start of `s`: the label, the url and the bytes used.
fn link(s: &str) -> Option<(&str, &str, usize)> {
    let close = s.find("](")?;
    let label = &s[1..close];
    if label.is_empty() || label.contains('[') {
        return None;
    }
    let after = &s[close + 2..];
    let end = after.find(')')?;
    let url = &after[..end];
    (!url.is_empty() && !url.contains(char::is_whitespace)).then_some((label, url, close + 2 + end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_bullets_bold_code_and_links_become_pango() {
        let md = "## What's new\n- **Fast** reader\n* see [notes](https://x.y/a?b=1&c=2)\nuse `a<b`";
        assert_eq!(
            to_pango(md),
            "<span size=\"x-large\" weight=\"bold\">What's new</span>\n• <b>Fast</b> reader\n• see <a href=\"https://x.y/a?b=1&amp;c=2\">notes</a>\nuse <tt>a&lt;b</tt>"
        );
    }

    #[test]
    fn markup_characters_are_escaped_and_unclosed_markers_stay_text() {
        assert_eq!(to_pango("a < b & c"), "a &lt; b &amp; c");
        assert_eq!(to_pango("**open"), "**open");
        assert_eq!(to_pango("[x](no close"), "[x](no close");
        assert_eq!(to_pango("#nospace"), "#nospace");
    }

    #[test]
    fn a_rule_line_is_dropped_and_an_empty_text_is_empty() {
        assert_eq!(to_pango("a\n---\nb"), "a\nb");
        assert_eq!(to_pango(""), "");
    }

    #[test]
    fn the_link_button_is_hidden_for_an_empty_link() {
        assert!(link_visible("https://github.com/o/r/releases/tag/v1"));
        assert!(!link_visible(""));
        assert!(!link_visible("  "));
    }

    /// The seven vectors of p1-edge-cases-and-tests section 2 (T-E10), as fixture files, through the release source that
    /// strips the checksum footer. Tests live inline because the binary crate has no library for `tests/` to call.
    #[test]
    fn the_seven_checksum_vectors_come_out_as_the_fixture_expects() {
        use kuro_domain::release::github::{GithubReleases, ReleaseHttp};
        use kuro_domain::release::ReleaseService;

        struct Body(String);
        impl ReleaseHttp for Body {
            fn get(&self, _url: &str, _agent: &str) -> Result<(u16, Vec<u8>), String> {
                let json = serde_json::json!({
                    "tag_name": "v1", "html_url": "https://x", "body": self.0,
                    "assets": [{ "name": "app", "browser_download_url": "https://y" }],
                });
                Ok((200, json.to_string().into_bytes()))
            }
        }

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/update");
        let mut seen = 0;
        for n in 1..=7 {
            let input = std::fs::read_to_string(dir.join(format!("changelog_{n:02}.md"))).unwrap();
            let expected = std::fs::read_to_string(dir.join(format!("changelog_{n:02}.expected.md"))).unwrap();
            let release = GithubReleases::new(Body(input), "x86_64").latest("o/r").unwrap().expect("a release");
            assert_eq!(release.info, expected, "vector {n}");
            seen += 1;
        }
        assert_eq!(seen, 7);
    }

    #[test]
    fn an_empty_changelog_renders_nothing() {
        assert_eq!(to_pango(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/update/changelog_07.md")).unwrap()), "");
    }
}
