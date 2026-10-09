//! Thin wrappers over `kuro_domain::items::recognition` (area:14 section 5). The algorithm belongs to area 07.
//!
//! The known-number argument is always `None`: Kotlin passes the SChapter / SEpisode default `-1`, and the algorithm
//! treats `-1` and `None` the same (both fall through to parsing the name).

use kuro_domain::items::recognition::{parse_chapter_number, parse_episode_number};

/// `ChapterRecognition.parseChapterNumber(mangaTitle, entryName, -1)`. `-1.0` when no number is found.
pub fn chapter_number(manga_title: &str, entry_name: &str) -> f32 {
    parse_chapter_number(manga_title, entry_name, None) as f32
}

/// `EpisodeRecognition.parseEpisodeNumber(animeTitle, fileStem, -1)`. `-1.0` when no number is found.
pub fn episode_number(anime_title: &str, file_stem: &str) -> f32 {
    parse_episode_number(anime_title, file_stem, None) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_numbers_come_from_the_shared_algorithm() {
        assert_eq!(chapter_number("Bleach", "Bleach 567 Down With Snowwhite"), 567.0);
        assert_eq!(chapter_number("Solanin", "Solanin 028.a Vol. 2"), 28.1);
        assert_eq!(chapter_number("random", "Foo"), -1.0);
    }

    #[test]
    fn episode_numbers_parse_the_usual_show_names() {
        assert_eq!(episode_number("Show", "Show - 01"), 1.0);
        assert_eq!(episode_number("Show", "Show - 12v2"), 12.0);
        assert_eq!(episode_number("Show", "Nothing here"), -1.0);
    }
}
