//! Chapter, episode and season number recognition from titles (`ChapterRecognition.kt`,
//! `EpisodeRecognition.kt`, `SeasonRecognition.kt`). Algorithm and vectors: `p1-topic-item-number-recognition.md`.
//!
//! The Kotlin regexes use look-behind; the `regex` crate has none, so each look-behind is rewritten as a
//! consuming prefix with the same capture groups. `\b` and `\s` are forced to ASCII to match Java.

use std::sync::LazyLock;

use regex::{Captures, Regex};

/// `NUMBER_PATTERN`: integer part, optional `.digits` decimal, optional alpha suffix.
const NUMBER: &str = r"([0-9]+)(\.[0-9]+)?(\.?[a-z]+)?";

/// `tagRegex`: leading or trailing `[...]` and `(...)` groups (episode and season only).
const TAG: &str = r"^\[[^\]]+\]|\[[^\]]+\](?-u:\s)*$|^\([^)]+\)|\([^)]+\)(?-u:\s)*$";

/// `unwantedWhiteSpace`, as a consuming rewrite: `\s(?=extra|special|omake)` becomes `\s(extra|special|omake)`
/// and is replaced by its keyword.
const UNWANTED_WHITESPACE: &str = r"(?-u:\s)(extra|special|omake)";

/// The three patterns of one recogniser.
struct Recogniser {
    /// Prefix form of the look-behind, groups 1-3 are the number parts.
    basic: Regex,
    /// `NUMBER` on its own.
    number: Regex,
    /// Tags that carry no number (`v2`, `ver002`, `1080p`, ...).
    unwanted: Regex,
    /// Bracketed tags stripped before matching. `None` for chapters.
    tag: Option<Regex>,
}

impl Recogniser {
    fn new(basic_prefix: &str, unwanted: &str, strip_tags: bool) -> Self {
        let compile = |p: &str| Regex::new(p).expect("static recogniser pattern");
        Self {
            basic: compile(&format!("{basic_prefix}{NUMBER}")),
            number: compile(NUMBER),
            unwanted: compile(unwanted),
            tag: strip_tags.then(|| compile(TAG)),
        }
    }
}

static UNWANTED_WS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(UNWANTED_WHITESPACE).expect("static pattern"));

static CHAPTER: LazyLock<Recogniser> = LazyLock::new(|| {
    Recogniser::new(
        r"ch\. *",
        r"(?-u:\b)(?:v|ver|vol|version|volume|season|s)[^a-z]?[0-9]+",
        false,
    )
});

static EPISODE: LazyLock<Recogniser> = LazyLock::new(|| {
    Recogniser::new(
        r"(?:(?-u:\b)e\.|(?-u:\b)e|episode|(?-u:\b)ep) *",
        r"(?-u:\b)(?:v|ver|version|season|s)[^a-z]?[0-9]+|(?-u:\b)[0-9]+p(?-u:\b)|hi10",
        true,
    )
});

static SEASON: LazyLock<Recogniser> = LazyLock::new(|| {
    Recogniser::new(
        r"(?:(?-u:\b)s\.|(?-u:\b)s|season) *",
        r"(?-u:\b)[0-9]+p(?-u:\b)|[0-9]+x[0-9]+|Hi10|\([0-9]+\)",
        true,
    )
});

/// `alphaPostfix`: `a` is 0.1 through `i` is 0.9, `j` and later are 0.0.
fn alpha_postfix(c: char) -> f64 {
    let n = u32::from(c).saturating_sub(u32::from('a')) + 1;
    if n >= 10 { 0.0 } else { f64::from(n) / 10.0 }
}

/// `checkForDecimal`: the fractional part of one match.
fn addition(decimal: Option<&str>, alpha: Option<&str>) -> f64 {
    if let Some(d) = decimal.filter(|d| !d.is_empty()) {
        return d.parse().unwrap_or(0.0);
    }
    if let Some(a) = alpha.filter(|a| !a.is_empty()) {
        if a.contains("extra") {
            return 0.99;
        }
        if a.contains("omake") {
            return 0.98;
        }
        if a.contains("special") {
            return 0.97;
        }
        let trimmed = a.trim_start_matches('.');
        let mut chars = trimmed.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return alpha_postfix(c);
        }
    }
    0.0
}

/// `getXNumberFromMatch`: integer part plus the fractional part, as one f64 sum.
fn value_of(caps: &Captures<'_>) -> f64 {
    // Group 1 is `[0-9]+`, so it always parses; the fallback only satisfies the type.
    let initial = caps
        .get(1)
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or(-1.0);
    let decimal = caps.get(2).map(|m| m.as_str());
    let alpha = caps.get(3).map(|m| m.as_str());
    initial + addition(decimal, alpha)
}

/// The 7-step algorithm of `p1-topic-item-number-recognition.md` section 2. `known` is returned as is when
/// it is `-2` or above `-1`.
fn parse(rec: &Recogniser, title: &str, name: &str, known: Option<f64>) -> f64 {
    if let Some(n) = known {
        if n == -2.0 || n > -1.0 {
            return n;
        }
    }

    let title_lc = title.to_lowercase();
    let mut s = name.to_lowercase();
    if !title_lc.is_empty() {
        s = s.replace(&title_lc, "");
    }
    let s = s.trim().replace([',', '-'], ".");
    let mut s = UNWANTED_WS.replace_all(&s, "$1").into_owned();

    if let Some(tag) = &rec.tag {
        // Each pass removes at least one non-empty match, so the loop ends.
        while tag.is_match(&s) {
            s = tag.replace_all(&s, "").into_owned();
        }
    }

    let mut matches = rec.number.captures_iter(&s);
    let Some(first) = matches.next() else {
        return known.unwrap_or(-1.0);
    };
    if matches.next().is_some() {
        let cleaned = rec.unwanted.replace_all(&s, "");
        if let Some(caps) = rec.basic.captures(&cleaned) {
            return value_of(&caps);
        }
        if let Some(caps) = rec.number.captures(&cleaned) {
            return value_of(&caps);
        }
    }
    value_of(&first)
}

/// `parseChapterNumber(mangaTitle, chapterName, chapterNumber)`.
pub fn parse_chapter_number(title: &str, name: &str, known: Option<f64>) -> f64 {
    parse(&CHAPTER, title, name, known)
}

/// `parseEpisodeNumber(animeTitle, episodeName, episodeNumber)`.
pub fn parse_episode_number(title: &str, name: &str, known: Option<f64>) -> f64 {
    parse(&EPISODE, title, name, known)
}

/// `parseSeasonNumber(animeTitle, seasonName, seasonNumber)`.
pub fn parse_season_number(title: &str, name: &str, known: Option<f64>) -> f64 {
    parse(&SEASON, title, name, known)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Upstream ChapterRecognitionTest vectors (`p1-topic-item-number-recognition.md` 5.1).
    #[test]
    fn upstream_chapter_vectors() {
        let alice = "Mokushiroku Alice";
        let bleach = "Bleach";
        let solanin = "Solanin";
        let onepunch = "Onepunch-Man";
        let rows: [(&str, &str, f64); 57] = [
            (
                alice,
                "Mokushiroku Alice Vol.1 Ch.4: Misrepresentation",
                4.0,
            ),
            (
                alice,
                "Mokushiroku Alice Vol. 1 Ch. 4: Misrepresentation",
                4.0,
            ),
            (
                alice,
                "Mokushiroku Alice Vol.1 Ch. 4: Misrepresentation",
                4.0,
            ),
            (
                alice,
                "Mokushiroku Alice Vol.1 Ch.4.1: Misrepresentation",
                4.1,
            ),
            (
                alice,
                "Mokushiroku Alice Vol.1 Ch.4.4: Misrepresentation",
                4.4,
            ),
            (
                alice,
                "Mokushiroku Alice Vol.1 Ch.4.a: Misrepresentation",
                4.1,
            ),
            (
                alice,
                "Mokushiroku Alice Vol.1 Ch.4.b: Misrepresentation",
                4.2,
            ),
            (
                alice,
                "Mokushiroku Alice Vol.1 Ch.4.extra: Misrepresentation",
                4.99,
            ),
            (bleach, "Bleach 567 Down With Snowwhite", 567.0),
            (bleach, "Bleach 567.1 Down With Snowwhite", 567.1),
            (bleach, "Bleach 567.4 Down With Snowwhite", 567.4),
            (bleach, "Bleach 567.a Down With Snowwhite", 567.1),
            (bleach, "Bleach 567.b Down With Snowwhite", 567.2),
            (bleach, "Bleach 567.extra Down With Snowwhite", 567.99),
            (solanin, "Solanin 028 Vol. 2", 28.0),
            (solanin, "Solanin 028.1 Vol. 2", 28.1),
            (solanin, "Solanin 028.4 Vol. 2", 28.4),
            (solanin, "Solanin 028.a Vol. 2", 28.1),
            (solanin, "Solanin 028.b Vol. 2", 28.2),
            (solanin, "Solanin 028.extra Vol. 2", 28.99),
            (onepunch, "Onepunch-Man Punch Ver002 028", 28.0),
            (onepunch, "Onepunch-Man Punch Ver002 028.1", 28.1),
            (onepunch, "Onepunch-Man Punch Ver002 028.4", 28.4),
            (onepunch, "Onepunch-Man Punch Ver002 028.a", 28.1),
            (onepunch, "Onepunch-Man Punch Ver002 028.b", 28.2),
            (onepunch, "Onepunch-Man Punch Ver002 028.extra", 28.99),
            ("random", "Vol.1 Ch.5v.2: Alones", 5.0),
            ("Ayame 14", "Ayame 14 1 - The summer of 14", 1.0),
            ("random", "Vol.1 Ch.1: March 25 (First Day Cohabiting)", 1.0),
            ("random", "Vol.001 Ch.003: Kaguya Doesn't Know Much", 3.0),
            (
                onepunch,
                "Onepunch-Man Punch Ver002 086 : Creeping Darkness [3]",
                86.0,
            ),
            (
                "Ansatsu Kyoushitsu",
                "Ansatsu Kyoushitsu 011v002: Assembly Time",
                11.0,
            ),
            ("Tokyo ESP", "Tokyo ESP 027: Part 002: Chapter 001", 27.0),
            ("One-punch Man", "Mag Version 195.5", 195.5),
            ("random", "Foo", -1.0),
            ("random", "Fairy Tail 404: 00:00", 404.0),
            ("random", "Asu No Yoichi 19a", 19.1),
            ("Fairy Tail", "Fairy Tail 404.extravol002", 404.99),
            ("Fairy Tail", "Fairy Tail 404 extravol002", 404.99),
            ("Fairy Tail", "Fairy Tail 404.omakevol002", 404.98),
            ("Fairy Tail", "Fairy Tail 404 omakevol002", 404.98),
            ("Fairy Tail", "Fairy Tail 404.specialvol002", 404.97),
            ("Fairy Tail", "Fairy Tail 404 specialvol002", 404.97),
            ("One Piece", "One Piece 300,a", 300.1),
            ("One Piece", "One Piece Ch,123,extra", 123.99),
            (
                "One Piece",
                "One Piece the sunny, goes swimming 024,005",
                24.005,
            ),
            ("Solo Leveling", "ch 122-a", 122.1),
            ("Solo Leveling", "Solo Leveling Ch.123-extra", 123.99),
            ("Solo Leveling", "Solo Leveling, 024-005", 24.005),
            ("Solo Leveling", "Ch.191-200 Read Online", 191.2),
            ("D.I.C.E", "D.I.C.E[Season 001] Ep. 007", 7.0),
            ("The Gamer", "S3 - Chapter 20", 20.0),
            ("One Outs", "One Outs 001", 1.0),
            (
                "The Sister of the Woods with a Thousand Young",
                "The 1st Night",
                1.0,
            ),
            (
                "The Sister of the Woods with a Thousand Young",
                "The 2nd Night",
                2.0,
            ),
            (
                "The Sister of the Woods with a Thousand Young",
                "The 3rd Night",
                3.0,
            ),
            (
                "The Sister of the Woods with a Thousand Young",
                "The 4th Night",
                4.0,
            ),
        ];
        for (title, name, expected) in rows {
            let got = parse_chapter_number(title, name, None);
            assert!(
                close(got, expected),
                "{name:?}: got {got}, expected {expected}"
            );
        }
    }

    #[test]
    fn chapter_known_shortcut_and_no_match() {
        assert_eq!(parse_chapter_number("X", "Foo", Some(-2.0)), -2.0);
        assert_eq!(parse_chapter_number("X", "Foo", None), -1.0);
        assert_eq!(
            parse_chapter_number("X", "Foo", Some(-1.5)),
            -1.5,
            "no match returns the known value"
        );
        assert_eq!(parse_chapter_number("X", "Foo 7", Some(-0.5)), -0.5);
        assert_eq!(parse_chapter_number("X", "Foo 7", Some(5.0)), 5.0);
        assert_eq!(parse_chapter_number("X", "Foo 7", Some(-2.0)), -2.0);
        assert_eq!(parse_chapter_number("X", "Foo 7", Some(0.0)), 0.0);
        assert!(
            close(parse_chapter_number("X", "Foo 7", Some(-1.5)), 7.0),
            "-1.5 is parsed, not returned"
        );
    }

    /// Additional chapter vectors (`p1-topic-item-number-recognition.md` 5.2).
    #[test]
    fn additional_chapter_vectors() {
        assert!(
            close(parse_chapter_number("X", "Ch.4.j", None), 4.0),
            "j is not a single-letter suffix"
        );
        assert!(close(parse_chapter_number("X", "Ch.4.i", None), 4.9));
        assert!(
            close(parse_chapter_number("X", "Ch.4.ab", None), 4.0),
            "two letters, not a keyword"
        );
        assert!(close(parse_chapter_number("X", "Chapter 12", None), 12.0));
        assert!(close(parse_chapter_number("X", "12", None), 12.0));
        assert!(close(parse_chapter_number("X", "Ch.0", None), 0.0));
        assert!(close(
            parse_chapter_number("X", "Vol.2 Ch.10 Part 3", None),
            10.0
        ));
        assert!(close(parse_chapter_number("X", "Ch.10-15", None), 10.15));
        assert!(close(
            parse_chapter_number("Ayame", "Ayame 14 - 7", None),
            14.0
        ));
        assert!(
            close(parse_chapter_number("", "Ch. 5", None), 5.0),
            "empty title is a no-op"
        );
    }

    /// Episode vectors (`p1-topic-item-number-recognition.md` 5.3).
    #[test]
    fn episode_vectors() {
        let kaguya = "kaguya-sama wa kokurasetai";
        let rows: [(&str, &str, f64); 20] = [
            (
                kaguya,
                "kaguya-sama wa kokurasetai - s01e01v2 (BD 1080p HEVC)",
                1.0,
            ),
            (
                "Kaguya",
                "[Flugel] Kaguya-sama wa Kokurasetai - s01e01v2 (BD 1080p HEVC) [Multi Audio] [80AC7B2E]",
                1.0,
            ),
            ("Show", "Show - Episode 12", 12.0),
            ("Show", "Show Ep 12.5", 12.5),
            ("Show", "Show E07", 7.0),
            ("Show", "Show - 03 [1080p]", 3.0),
            ("Show", "Show 05 (BD 1080p)", 5.0),
            ("Show", "Show S2 Ep 3", 3.0),
            ("Show", "Foo", -1.0),
            ("Show", "Show 12 Hi10 1080p", 12.0),
            ("Show", "[Group] Show - 08v2 [720p]", 8.0),
            ("Show", "Show e5.5", 5.5),
            ("Show", "Show 1080p 03", 3.0),
            ("X", "X - 07v2", 7.0),
            ("X", "1080p", 1080.0),
            ("X", "S01E05", 5.0),
            ("X", "S01E05 [1080p]", 5.0),
            ("X", "Ep.12.5", 12.5),
            ("X", "(2020) Show 04", 4.0),
            ("Show", "Show Episode 5 special", 5.0),
        ];
        for (title, name, expected) in rows {
            if name == "Show Episode 5 special" {
                assert_eq!(
                    parse_episode_number(title, name, None),
                    5.0 + 0.97,
                    "{name}"
                );
                continue;
            }
            let got = parse_episode_number(title, name, None);
            assert!(
                close(got, expected),
                "{name:?}: got {got}, expected {expected}"
            );
        }
    }

    #[test]
    fn episode_known_shortcut_and_tags() {
        assert_eq!(parse_episode_number("x", "Foo", Some(5.0)), 5.0);
        assert_eq!(parse_episode_number("x", "Foo", Some(-2.0)), -2.0);
        assert_eq!(parse_episode_number("x", "Foo", Some(-1.0)), -1.0);
        assert_eq!(parse_episode_number("x", "Foo", Some(-0.5)), -0.5);
        assert_eq!(parse_episode_number("x", "Foo 3", Some(-1.0)), 3.0);
    }

    #[test]
    fn episode_tag_loop_strips_leading_and_trailing_brackets_until_none_remain() {
        assert_eq!(
            parse_episode_number("x", "[A] (B) [C] 09 [D] (E)", None),
            9.0
        );
        assert_eq!(parse_episode_number("x", "(2020) [HD] Show 04", None), 4.0);
    }

    /// Season vectors (`p1-topic-item-number-recognition.md` 5.4).
    #[test]
    fn season_vectors() {
        let bnha = "Boku no Hero Academia";
        let rows: [(&str, &str, f64); 18] = [
            (bnha, "Boku no Hero Academia Season 2", 2.0),
            (
                bnha,
                "[FLE] Boku no Hero Academia Season 2 (BD Remux 1080p H.264 FLAC) [Dual Audio]",
                2.0,
            ),
            (bnha, "Boku.no.Hero.Academia.S02.1080p-ITH", 2.108),
            (bnha, "Boku no Hero Academia 2", 2.0),
            (bnha, "Boku no Hero Academia (2017)", -1.0),
            ("Show", "Show Season 3 Part 2", 3.0),
            ("Show", "Show S3", 3.0),
            ("Show", "Show: The Movie", -1.0),
            ("Show", "Show 2nd Season", 2.0),
            ("Show", "Show Hi10 s02", 2.0),
            ("Show", "Show s.2", 2.0),
            ("Show", "Show season 1.5", 1.5),
            ("Show", "Show 1920x1080 Season 4", 4.0),
            ("X", "X S2", 2.0),
            ("X", "Season 2.5", 2.5),
            ("X", "X (2019)", -1.0),
            ("X", "X 2nd Season 1080p", 2.0),
            ("X", "X Part 2", 2.0),
        ];
        for (title, name, expected) in rows {
            let got = parse_season_number(title, name, None);
            assert!(
                close(got, expected),
                "{name:?}: got {got}, expected {expected}"
            );
        }
        assert_eq!(
            parse_season_number(
                "Boku no Hero Academia",
                "Boku no Hero Academia (2017)",
                None
            ),
            -1.0
        );
    }

    #[test]
    fn season_known_shortcut_matches_chapters() {
        assert_eq!(parse_season_number("x", "Foo", Some(-2.0)), -2.0);
        assert_eq!(parse_season_number("x", "Foo", Some(-1.5)), -1.5);
        assert_eq!(parse_season_number("x", "Foo 2", Some(-0.5)), -0.5);
    }

    #[test]
    fn quirk_q5_capital_hi10_never_matches_lowercased_input() {
        // The season unwanted list has `Hi10`, the input is lowercased first, so the branch never fires.
        assert_eq!(parse_season_number("Show", "Show Hi10 s02", None), 2.0);
    }

    #[test]
    fn non_latin_titles_keep_ascii_word_boundaries() {
        // A kana directly before `s2` must not move the boundary: `\b` stays ASCII.
        assert_eq!(parse_chapter_number("X", "ありがとうs2 ch.9", None), 9.0);
        assert!(close(parse_chapter_number("X", "é5", None), 5.0));
    }

    #[test]
    fn ten_thousand_names_parse_quickly() {
        let names: Vec<String> = (0..10_000)
            .map(|i| format!("Series Name Ch.{i}.5 [Group] (2020)"))
            .collect();
        let started = std::time::Instant::now();
        let total: f64 = names
            .iter()
            .map(|n| parse_chapter_number("Series Name", n, None))
            .sum();
        assert!(total.is_finite());
        // The acceptance bound is 100 ms in release; debug builds get a loose sanity bound.
        let bound = if cfg!(debug_assertions) {
            std::time::Duration::from_secs(5)
        } else {
            std::time::Duration::from_millis(100)
        };
        assert!(
            started.elapsed() < bound,
            "10 000 names took {:?}",
            started.elapsed()
        );
    }
}
