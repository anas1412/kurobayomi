//! `@mention` links in release notes (`ReleaseServiceImpl.kt`). Same rule as the upstream pattern
//! `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))` with ignore-case, written as a scanner so no
//! look-around engine is needed (C-07).

/// Longest GitHub user name, in characters, with the `@` left out.
const MAX_NAME: usize = 39;

/// Wraps every `@name` in `text` as `[@name](https://github.com/name)`. A name is an ASCII letter or digit, then up to
/// 38 more characters where a single `-` may sit between two letters or digits, and it ends in a letter or digit. The
/// `@` must not follow a word character, so `me@host` is left alone.
pub fn linkify_mentions(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let alnum = |c: char| c.is_ascii_alphanumeric();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let starts = chars[i] == '@'
            && (i == 0 || !word(chars[i - 1]))
            && chars.get(i + 1).is_some_and(|&c| alnum(c));
        if !starts {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // `end` is the exclusive end of the longest valid name that starts at `i + 1`.
        let mut end = i + 2;
        let mut k = i + 2;
        let mut len = 1;
        while k < chars.len() && len < MAX_NAME {
            if alnum(chars[k]) {
                k += 1;
                len += 1;
                end = k;
            } else if chars[k] == '-' && len + 2 <= MAX_NAME && chars.get(k + 1).is_some_and(|&c| alnum(c)) {
                k += 2;
                len += 2;
                end = k;
            } else {
                break;
            }
        }
        let name: String = chars[i + 1..end].iter().collect();
        out.push_str(&format!("[@{name}](https://github.com/{name})"));
        i = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_upstream_vectors_hold() {
        assert_eq!(linkify_mentions("Thanks @bob!"), "Thanks [@bob](https://github.com/bob)!");
        assert_eq!(linkify_mentions("@a-b-c"), "[@a-b-c](https://github.com/a-b-c)");
        assert_eq!(linkify_mentions("@a--b"), "[@a](https://github.com/a)--b");
        assert_eq!(linkify_mentions("@a-"), "[@a](https://github.com/a)-");
        assert_eq!(linkify_mentions("mail me@host"), "mail me@host");
        assert_eq!(linkify_mentions("x_@bob"), "x_@bob");
        assert_eq!(linkify_mentions("@Bob"), "[@Bob](https://github.com/Bob)");
        assert_eq!(linkify_mentions("**@bob**"), "**[@bob](https://github.com/bob)**");
        assert_eq!(linkify_mentions("(@bob)"), "([@bob](https://github.com/bob))");
    }

    #[test]
    fn a_name_is_cut_at_39_characters() {
        let long = "a".repeat(45);
        let out = linkify_mentions(&format!("@{long}"));
        let name = "a".repeat(39);
        assert_eq!(out, format!("[@{name}](https://github.com/{name}){}", "a".repeat(6)));
    }

    #[test]
    fn a_lone_at_or_a_name_that_cannot_start_is_plain_text() {
        assert_eq!(linkify_mentions("@"), "@");
        assert_eq!(linkify_mentions("@-bob"), "@-bob");
        assert_eq!(linkify_mentions(""), "");
    }
}
