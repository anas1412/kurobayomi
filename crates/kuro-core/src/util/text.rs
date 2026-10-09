//! Text helpers (port of `StringExtensions.kt` and `SortUtil.kt`).
//!
//! Lengths are counted in UTF-16 units like Kotlin, but a cut never splits a character.

use std::cmp::Ordering;
use std::sync::OnceLock;

use icu_collator::options::{CollatorOptions, Strength};
use icu_collator::{Collator, CollatorBorrowed};
use icu_locale::Locale;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TextError {
    #[error("replacement is longer than the requested length")]
    ReplacementTooLong,
}

fn units(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// Longest prefix of `s` with at most `n` UTF-16 units.
pub fn take_utf16(s: &str, n: usize) -> &str {
    let mut used = 0;
    for (i, c) in s.char_indices() {
        used += c.len_utf16();
        if used > n {
            return &s[..i];
        }
    }
    s
}

/// Longest suffix of `s` with at most `n` UTF-16 units.
fn take_last_utf16(s: &str, n: usize) -> &str {
    let mut used = 0;
    for (i, c) in s.char_indices().rev() {
        used += c.len_utf16();
        if used > n {
            return &s[i + c.len_utf8()..];
        }
    }
    s
}

/// Longest prefix of `s` with at most `n` bytes, cut on a character boundary.
pub fn take_bytes(s: &str, n: usize) -> &str {
    let mut end = n.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Keeps at most `count` UTF-16 units: a longer string becomes its prefix plus `replacement`.
pub fn chop(s: &str, count: usize, replacement: &str) -> Result<String, TextError> {
    let repl = units(replacement);
    if repl > count {
        return Err(TextError::ReplacementTooLong);
    }
    if units(s) <= count {
        return Ok(s.to_owned());
    }
    Ok(format!("{}{replacement}", take_utf16(s, count - repl)))
}

/// Keeps at most `count` UTF-16 units: a longer string becomes its head plus `replacement`
/// plus its tail, each `(count - replacement) / 2` units long.
pub fn truncate_center(s: &str, count: usize, replacement: &str) -> Result<String, TextError> {
    let repl = units(replacement);
    if repl > count {
        return Err(TextError::ReplacementTooLong);
    }
    if units(s) <= count {
        return Ok(s.to_owned());
    }
    let piece = (count - repl) / 2;
    Ok(format!(
        "{}{replacement}{}",
        take_utf16(s, piece),
        take_last_utf16(s, piece)
    ))
}

/// Case-insensitive natural order over UTF-16 units. Digit runs compare by their numeric
/// value in wrapping `u64` arithmetic, digits sort before other characters, and other units
/// compare after simple per-unit lowercasing (port of `CaseInsensitiveSimpleNaturalComparator`).
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let a: Vec<u16> = a.encode_utf16().collect();
    let b: Vec<u16> = b.encode_utf16().collect();
    let (mut i, mut j) = (0, 0);
    loop {
        match (a.get(i), b.get(j)) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&x), Some(&y)) => {
                let (dx, dy) = (is_digit(x), is_digit(y));
                if dx && dy {
                    let (vx, next_i) = digit_run(&a, i);
                    let (vy, next_j) = digit_run(&b, j);
                    if vx != vy {
                        return vx.cmp(&vy);
                    }
                    (i, j) = (next_i, next_j);
                } else if dx != dy {
                    return if dx {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                } else {
                    let (lx, ly) = (lower_unit(x), lower_unit(y));
                    if lx != ly {
                        return lx.cmp(&ly);
                    }
                    (i, j) = (i + 1, j + 1);
                }
            }
        }
    }
}

fn is_digit(unit: u16) -> bool {
    (u16::from(b'0')..=u16::from(b'9')).contains(&unit)
}

/// Numeric value of the digit run starting at `start`, and the index just past it.
fn digit_run(units: &[u16], start: usize) -> (u64, usize) {
    let mut value: u64 = 0;
    let mut i = start;
    while let Some(&unit) = units.get(i).filter(|&&u| is_digit(u)) {
        value = value
            .wrapping_mul(10)
            .wrapping_add(u64::from(unit - u16::from(b'0')));
        i += 1;
    }
    (value, i)
}

/// Simple lowercase of one UTF-16 unit. Surrogates and multi-char mappings stay unchanged,
/// except U+0130: Java's `Character.toLowerCase` gives `i`, while Rust's full mapping is `i` plus
/// a combining dot, which would leave the unit unchanged.
fn lower_unit(unit: u16) -> u16 {
    if unit < 128 {
        return u16::from((unit as u8).to_ascii_lowercase());
    }
    if unit == 0x0130 {
        return u16::from(b'i');
    }
    let Some(c) = char::from_u32(u32::from(unit)) else {
        return unit;
    };
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) if (l as u32) <= 0xFFFF => l as u16,
        _ => unit,
    }
}

/// Primary-strength collator, built once from the environment locale and falling back to `en`.
static COLLATOR: OnceLock<CollatorBorrowed<'static>> = OnceLock::new();

/// Converts a POSIX locale value (`de_DE.UTF-8`) to a BCP 47 tag (`de-DE`). `C` and `POSIX`
/// have no tag.
fn posix_to_bcp47(value: &str) -> Option<String> {
    let base = value.split(['.', '@']).next()?;
    if base.is_empty() || base == "C" || base == "POSIX" {
        return None;
    }
    Some(base.replace('_', "-"))
}

/// Primary-strength collator for `tag`, or `None` when the tag does not parse.
fn build_collator(tag: Option<&str>) -> Option<CollatorBorrowed<'static>> {
    let locale = Locale::try_from_str(tag?).ok()?;
    let mut options = CollatorOptions::default();
    options.strength = Some(Strength::Primary);
    Collator::try_new(locale.into(), options).ok()
}

fn collator() -> &'static CollatorBorrowed<'static> {
    COLLATOR.get_or_init(|| {
        let env = ["LC_ALL", "LC_COLLATE", "LANG"]
            .iter()
            .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()));
        let tag = env.as_deref().and_then(posix_to_bcp47);
        build_collator(tag.as_deref()).unwrap_or_else(|| {
            build_collator(Some("en")).expect("en collation data is compiled in")
        })
    })
}

/// Compares two strings at primary strength: `a` equals `A` and `á`, and `b` is after `a`.
pub fn primary_cmp(a: &str, b: &str) -> Ordering {
    collator().compare(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chop_matches_spec_example() {
        assert_eq!(chop("abcdef", 5, "…").unwrap(), "abcd…");
        assert_eq!(chop("abc", 5, "…").unwrap(), "abc");
        assert_eq!(chop("abc", 3, "…").unwrap(), "abc");
    }

    #[test]
    fn chop_counts_utf16_units_and_never_splits_characters() {
        // "😀" is two UTF-16 units: a 2-unit budget before "…" keeps "a" but not the emoji.
        assert_eq!(chop("a😀bc", 3, "…").unwrap(), "a…");
        assert_eq!(chop("😀😀😀", 5, "…").unwrap(), "😀😀…");
        assert_eq!(chop("日本語テキスト", 4, "…").unwrap(), "日本語…");
    }

    #[test]
    fn replacement_longer_than_limit_is_an_error() {
        assert_eq!(chop("abcdef", 0, "…"), Err(TextError::ReplacementTooLong));
        assert_eq!(chop("ab", 1, "..."), Err(TextError::ReplacementTooLong));
        assert_eq!(
            truncate_center("abcdefghij", 2, "..."),
            Err(TextError::ReplacementTooLong)
        );
    }

    #[test]
    fn truncate_center_matches_spec_example() {
        assert_eq!(truncate_center("abcdefghij", 7, "...").unwrap(), "ab...ij");
        assert_eq!(truncate_center("abc", 7, "...").unwrap(), "abc");
    }

    #[test]
    fn truncate_center_with_odd_budget_and_multibyte() {
        // (8 - 3) / 2 = 2 units on each side.
        assert_eq!(truncate_center("abcdefghij", 8, "...").unwrap(), "ab...ij");
        assert_eq!(
            truncate_center("日本語テキスト", 5, "…").unwrap(),
            "日本…スト"
        );
        assert_eq!(truncate_center("a😀b😀c", 3, "…").unwrap(), "a…c");
    }

    #[test]
    fn take_bytes_stops_on_character_boundary() {
        assert_eq!(take_bytes("日本語", 4), "日");
        assert_eq!(take_bytes("日本語", 9), "日本語");
        assert_eq!(take_bytes("日本語", 100), "日本語");
        assert_eq!(take_bytes("a\u{FFFD}b", 2), "a");
        assert_eq!(take_bytes("a\u{FFFD}b", 4), "a\u{FFFD}");
    }

    #[test]
    fn take_utf16_never_splits_a_pair() {
        assert_eq!(take_utf16("a😀", 1), "a");
        assert_eq!(take_utf16("a😀", 3), "a😀");
    }

    fn sign(a: &str, b: &str) -> i32 {
        natural_cmp(a, b) as i32
    }

    #[test]
    fn natural_order_vectors() {
        assert_eq!(sign("file2", "file10"), -1);
        assert_eq!(sign("file10", "file2"), 1);
        assert_eq!(sign("a", "B"), -1);
        assert_eq!(sign("b", "A"), 1);
        assert_eq!(sign("01a", "1b"), -1);
        assert_eq!(sign("1", "a"), -1);
        assert_eq!(sign("a", "1"), 1);
        assert_eq!(sign("Chapter 1", "chapter 2"), -1);
        assert_eq!(sign("x9999999999999999999999", "x1"), 1);
        assert_eq!(sign("img001", "img1"), 0);
        assert_eq!(sign("\u{130}", "i"), 0);
        assert_eq!(sign("\u{130}a", "ib"), -1);
        assert_eq!(sign("", "a"), -1);
        assert_eq!(sign("a", ""), 1);
    }

    #[test]
    fn natural_order_is_antisymmetric_on_random_ascii() {
        // Deterministic LCG in place of proptest: 10,000 pairs of short ASCII strings.
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        let alphabet = b"aAbB09 .x";
        let mut word = || -> String {
            let len = next() % 8;
            (0..len)
                .map(|_| alphabet[next() % alphabet.len()] as char)
                .collect()
        };
        for _ in 0..10_000 {
            let (a, b) = (word(), word());
            assert_eq!(
                natural_cmp(&a, &b),
                natural_cmp(&b, &a).reverse(),
                "{a:?} vs {b:?}"
            );
        }
    }

    #[test]
    fn primary_collation_ignores_case_and_accents() {
        assert_eq!(primary_cmp("a", "A"), Ordering::Equal);
        assert_eq!(primary_cmp("a", "á"), Ordering::Equal);
        assert_eq!(primary_cmp("a", "b"), Ordering::Less);
        assert_eq!(primary_cmp("Ä", "b"), Ordering::Less);
    }

    #[test]
    fn primary_sort_is_stable_on_ties() {
        let mut names = vec!["b", "A", "á", "c"];
        names.sort_by(|a, b| primary_cmp(a, b));
        assert_eq!(names, vec!["A", "á", "b", "c"]);
    }

    /// Independent formulation of the section 2.1 reference: each string becomes tokens (a digit
    /// run as its value, or one lowercased character), digits sort before characters, and a
    /// shorter token list sorts first.
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum Token {
        Number(u64),
        Char(u16),
    }

    fn tokens(s: &str) -> Vec<Token> {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < units.len() {
            if is_digit(units[i]) {
                let (value, next) = digit_run(&units, i);
                out.push(Token::Number(value));
                i = next;
            } else {
                out.push(Token::Char(lower_unit(units[i])));
                i += 1;
            }
        }
        out
    }

    fn natural_ref(a: &str, b: &str) -> Ordering {
        tokens(a).cmp(&tokens(b))
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(10_000))]

        #[test]
        fn natural_order_agrees_with_reference_on_random_ascii(a in "[aAbBzZ09 .-]{0,12}", b in "[aAbBzZ09 .-]{0,12}") {
            proptest::prop_assert_eq!(natural_cmp(&a, &b), natural_ref(&a, &b));
        }
    }

    #[test]
    fn collation_vectors_en_and_de() {
        for tag in ["en", "de-DE"] {
            let c = build_collator(Some(tag)).unwrap();
            assert_eq!(c.compare("a", "A"), Ordering::Equal, "{tag}");
            assert_eq!(c.compare("a", "á"), Ordering::Equal, "{tag}");
            assert_eq!(c.compare("a", "b"), Ordering::Less, "{tag}");
            assert_eq!(c.compare("Ä", "b"), Ordering::Less, "{tag}");
        }
    }

    #[test]
    fn collation_follows_the_locale_tailoring() {
        // Swedish sorts ä after z; English treats it as a variant of a.
        assert_eq!(build_collator(Some("sv")).unwrap().compare("ä", "z"), Ordering::Greater);
        assert_eq!(build_collator(Some("en")).unwrap().compare("ä", "z"), Ordering::Less);
    }

    #[test]
    fn posix_locale_values_map_to_tags() {
        assert_eq!(posix_to_bcp47("de_DE.UTF-8").as_deref(), Some("de-DE"));
        assert_eq!(posix_to_bcp47("sv_SE@euro").as_deref(), Some("sv-SE"));
        assert_eq!(posix_to_bcp47("C"), None);
        assert_eq!(posix_to_bcp47("POSIX"), None);
        assert_eq!(posix_to_bcp47(""), None);
    }

    #[test]
    fn unparseable_locale_has_no_collator_so_callers_use_en() {
        assert!(build_collator(None).is_none());
        assert!(build_collator(Some("not a locale!!")).is_none());
        assert_eq!(collator().compare("a", "A"), Ordering::Equal);
    }
}
