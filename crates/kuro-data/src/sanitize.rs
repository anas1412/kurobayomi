//! Chapter and episode name sanitizer (`ChapterSanitizer.kt`, `EpisodeSanitizer.kt`, T-05a-009). The two
//! Kotlin objects are the same algorithm with the same 30-entry trim array, so one function serves both.
//! Pure: no allocation beyond the returned string.

/// The characters stripped from both ends in step 3: the Kotlin whitespace list without U+001C-U+001F,
/// plus `-`, `_`, `,` and `:`. The 30 array entries of the Kotlin source hold 29 distinct characters.
pub const TRIM_CHARS: [char; 29] = [
    '\u{0020}', '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0085}', '\u{00A0}', '\u{1680}',
    '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}',
    '\u{2009}', '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}', '-', '_', ',', ':',
];

/// Kotlin `Char.isWhitespace()`: `Character.isWhitespace || Character.isSpaceChar`. Differs from Rust's
/// `char::is_whitespace` in both directions (U+001C-U+001F in, U+0085 out).
fn is_kotlin_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'..='\u{000D}'
            | '\u{001C}'..='\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

/// `sChapter.name.sanitize(manga.title)` and `sEpisode.name.sanitize(anime.title)`:
/// 1. trim Kotlin whitespace, 2. drop one case-sensitive `title` prefix, 3. trim [`TRIM_CHARS`] from both ends.
pub fn sanitize_item_name(name: &str, title: &str) -> String {
    let trimmed = name.trim_matches(is_kotlin_whitespace);
    // An empty title is a no-op prefix, as `removePrefix("")` is.
    let without_title = trimmed.strip_prefix(title).unwrap_or(trimmed);
    without_title.trim_matches(|c| TRIM_CHARS.contains(&c)).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The behaviour table of `p1-topic-item-sanitizer.md` section 3.
    #[test]
    fn behaviour_table_vectors() {
        let cases: &[(&str, &str, &str)] = &[
            ("One Piece 1000", "One Piece", "1000"),
            ("One Piece - Chapter 12", "One Piece", "Chapter 12"),
            ("Chapter 5 -", "X", "Chapter 5"),
            ("_-:Special,", "", "Special"),
            ("One Piece", "One Piece", ""),
            ("one piece 3", "One Piece", "one piece 3"),
            ("  One Piece 3", "One Piece", "3"),
            ("One Piece  One Piece 3", "One Piece", "One Piece 3"),
            ("\u{00A0}Ch 1\u{3000}", "X", "Ch 1"),
            ("Ch. 1.", "X", "Ch. 1."),
            ("\u{001F}One Piece 1", "One Piece", "1"),
        ];
        for (name, title, want) in cases {
            assert_eq!(sanitize_item_name(name, title), *want, "name {name:?} title {title:?}");
        }
    }

    #[test]
    fn trim_set_has_29_distinct_characters() {
        let mut sorted = TRIM_CHARS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 29);
    }

    #[test]
    fn kotlin_whitespace_includes_the_separator_controls_and_excludes_next_line() {
        assert!(is_kotlin_whitespace('\u{001F}'));
        assert!(is_kotlin_whitespace('\u{3000}'));
        assert!(!is_kotlin_whitespace('\u{0085}'));
        assert!(!is_kotlin_whitespace('.'));
    }

    #[test]
    fn full_width_text_does_not_match_the_ascii_title() {
        assert_eq!(sanitize_item_name("\u{FF2F}ne Piece 2", "One Piece"), "\u{FF2F}ne Piece 2");
    }
}
