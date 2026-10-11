//! Chapter and episode name sanitizer (`ChapterSanitizer.kt`, `EpisodeSanitizer.kt`, T-05a-009). The two
//! Kotlin objects are the same algorithm with the same 30-entry trim array, so one function serves both.
//! Pure: no allocation beyond the returned string. T-05a-009, T-05a-016.

pub use kuro_db::episode_sanitizer::TRIM_CHARS;

/// `sChapter.name.sanitize(manga.title)` and `sEpisode.name.sanitize(anime.title)`:
/// 1. trim Kotlin whitespace, 2. drop one case-sensitive `title` prefix, 3. trim [`TRIM_CHARS`] from both ends.
/// The one implementation lives in `kuro_db::episode_sanitizer` (kuro-db sits below this crate); chapter sync,
/// episode sync and the episode wrapper all reach it through this function.
pub fn sanitize_item_name(name: &str, title: &str) -> String {
    kuro_db::episode_sanitizer::sanitize(name, title)
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

    /// T-05a-016: the episode wrapper is the chapter function, so the vectors and every trim character agree.
    #[test]
    fn the_episode_wrapper_gives_the_same_results_as_the_item_function() {
        use crate::episode::sanitize_episode_name;
        let cases = [("One Piece - 12", "One Piece"), ("\u{001F}One Piece 1", "One Piece"), ("_-:Special,", ""), ("One Piece", "One Piece")];
        for (name, title) in cases {
            assert_eq!(sanitize_episode_name(name, title), sanitize_item_name(name, title), "{name:?}");
        }
        for c in TRIM_CHARS {
            let padded = format!("{c}x{c}");
            assert_eq!(sanitize_item_name(&padded, ""), "x", "U+{:04X}", c as u32);
            assert_eq!(sanitize_episode_name(&padded, ""), "x", "U+{:04X}", c as u32);
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
    fn full_width_text_does_not_match_the_ascii_title() {
        assert_eq!(sanitize_item_name("\u{FF2F}ne Piece 2", "One Piece"), "\u{FF2F}ne Piece 2");
    }
}
