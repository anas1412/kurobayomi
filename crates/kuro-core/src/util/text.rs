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

/// Device names that Windows refuses as file names (addendum rule 5).
const RESERVED_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The one file-name sanitiser: `DiskUtil.buildValidFilename` (trim leading and trailing dots and spaces, map control
/// and FAT-forbidden characters to `_`, `(invalid)` for an empty result, keep at most 240 UTF-16 units).
///
/// With `ntfs` the NTFS rules of the cross-platform addendum (rule 5) apply afterwards: a trailing dot or space left
/// by the 240-unit cut is trimmed again, and a reserved device name gets `_` before its extension (`CON` becomes
/// `CON_`). Without it the output equals the Kotlin output exactly.
pub fn build_valid_filename(orig: &str, ntfs: bool) -> String {
    let trimmed = orig.trim_matches(|c| c == '.' || c == ' ');
    if trimmed.is_empty() {
        return "(invalid)".to_owned();
    }
    let mapped: String = trimmed
        .chars()
        .map(|c| if c <= '\u{1f}' || c == '\u{7f}' || "\"*/:<>?\\|".contains(c) { '_' } else { c })
        .collect();
    let cut = take_utf16(&mapped, 240);
    if !ntfs {
        return cut.to_owned();
    }
    let cut = cut.trim_end_matches(['.', ' ']);
    let stem_end = cut.find('.').unwrap_or(cut.len());
    let stem = cut[..stem_end].trim_end_matches(' ');
    if RESERVED_DEVICE_NAMES.iter().any(|r| stem.eq_ignore_ascii_case(r)) {
        format!("{stem}_{}", &cut[stem_end..])
    } else {
        cut.to_owned()
    }
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

/// UTF-8 length of `s` in bytes (`String.byteSize`).
pub fn byte_size(s: &str) -> usize {
    s.len()
}

/// Names of U+00A0 to U+00FF in order (HTML 4 Latin-1 entities).
const LATIN1: [&str; 96] = [
    "nbsp", "iexcl", "cent", "pound", "curren", "yen", "brvbar", "sect", "uml", "copy", "ordf",
    "laquo", "not", "shy", "reg", "macr", "deg", "plusmn", "sup2", "sup3", "acute", "micro", "para",
    "middot", "cedil", "sup1", "ordm", "raquo", "frac14", "frac12", "frac34", "iquest", "Agrave",
    "Aacute", "Acirc", "Atilde", "Auml", "Aring", "AElig", "Ccedil", "Egrave", "Eacute", "Ecirc",
    "Euml", "Igrave", "Iacute", "Icirc", "Iuml", "ETH", "Ntilde", "Ograve", "Oacute", "Ocirc",
    "Otilde", "Ouml", "times", "Oslash", "Ugrave", "Uacute", "Ucirc", "Uuml", "Yacute", "THORN",
    "szlig", "agrave", "aacute", "acirc", "atilde", "auml", "aring", "aelig", "ccedil", "egrave",
    "eacute", "ecirc", "euml", "igrave", "iacute", "icirc", "iuml", "eth", "ntilde", "ograve",
    "oacute", "ocirc", "otilde", "ouml", "divide", "oslash", "ugrave", "uacute", "ucirc", "uuml",
    "yacute", "thorn", "yuml",
];

/// ponytail: Latin-1 plus the common punctuation and symbol entities; the Greek and maths entities of HTML 4 are
/// not in the table and stay as written. Add them when a tracker payload needs them.
fn named_entity(name: &str) -> Option<char> {
    if let Some(i) = LATIN1.iter().position(|n| *n == name) {
        return char::from_u32(0xA0 + i as u32);
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "OElig" => '\u{152}',
        "oelig" => '\u{153}',
        "Scaron" => '\u{160}',
        "scaron" => '\u{161}',
        "Yuml" => '\u{178}',
        "fnof" => '\u{192}',
        "circ" => '\u{2C6}',
        "tilde" => '\u{2DC}',
        "ensp" => '\u{2002}',
        "emsp" => '\u{2003}',
        "thinsp" => '\u{2009}',
        "ndash" => '\u{2013}',
        "mdash" => '\u{2014}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "sbquo" => '\u{201A}',
        "ldquo" => '\u{201C}',
        "rdquo" => '\u{201D}',
        "bdquo" => '\u{201E}',
        "dagger" => '\u{2020}',
        "Dagger" => '\u{2021}',
        "bull" => '\u{2022}',
        "hellip" => '\u{2026}',
        "permil" => '\u{2030}',
        "prime" => '\u{2032}',
        "Prime" => '\u{2033}',
        "lsaquo" => '\u{2039}',
        "rsaquo" => '\u{203A}',
        "euro" => '\u{20AC}',
        "trade" => '\u{2122}',
        "larr" => '\u{2190}',
        "uarr" => '\u{2191}',
        "rarr" => '\u{2192}',
        "darr" => '\u{2193}',
        "harr" => '\u{2194}',
        "minus" => '\u{2212}',
        "infin" => '\u{221E}',
        "ne" => '\u{2260}',
        "le" => '\u{2264}',
        "ge" => '\u{2265}',
        "hearts" => '\u{2665}',
        _ => return None,
    })
}

/// Decodes one `&...;` reference at the start of `rest` (which begins with `&`) into `out` and returns the text
/// after it. Anything unknown is written as a literal `&`.
fn push_entity<'a>(rest: &'a str, out: &mut String) -> &'a str {
    let decoded = rest
        .char_indices()
        .take(34)
        .find(|&(_, c)| c == ';')
        .and_then(|(semi, _)| {
            let body = &rest[1..semi];
            let c = match body.strip_prefix('#') {
                Some(num) => {
                    let code = match num.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16),
                        None => num.parse(),
                    };
                    code.ok().filter(|&n| n != 0).and_then(char::from_u32)
                }
                None => named_entity(body),
            };
            c.map(|c| (c, semi + 1))
        });
    match decoded {
        Some((c, end)) => {
            out.push(c);
            &rest[end..]
        }
        None => {
            out.push('&');
            &rest[1..]
        }
    }
}

/// Handles the `<` at the start of `rest`: writes any text the tag stands for into `out` and returns the text
/// after the tag. A `<` that does not start a tag is written as itself.
fn skip_tag<'a>(rest: &'a str, out: &mut String) -> &'a str {
    if let Some(comment) = rest.strip_prefix("<!--") {
        return comment.find("-->").map_or("", |i| &comment[i + 3..]);
    }
    let body = &rest[1..];
    let closing = body.starts_with('/');
    let name_part = body.strip_prefix('/').unwrap_or(body);
    if !name_part.starts_with(|c: char| c.is_ascii_alphabetic() || c == '!' || c == '?') && !closing {
        out.push('<');
        return body;
    }
    // The end of the tag: the first `>` outside quotes.
    let mut quote = None;
    let end = body.char_indices().find_map(|(i, c)| match (quote, c) {
        (None, '"' | '\'') => {
            quote = Some(c);
            None
        }
        (Some(q), c) if c == q => {
            quote = None;
            None
        }
        (None, '>') => Some(i),
        _ => None,
    });
    let Some(end) = end else {
        out.push('<');
        return body;
    };
    let after = &body[end + 1..];
    let name: String = name_part
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    match (closing, name.as_str()) {
        (_, "br") | (true, "p" | "div" | "li") => out.push('\n'),
        (false, "script" | "style") if !body[..end].ends_with('/') => {
            let close = format!("</{name}");
            return match after.to_ascii_lowercase().find(&close) {
                Some(i) => after[i..].find('>').map_or("", |j| &after[i + j + 1..]),
                None => "",
            };
        }
        _ => {}
    }
    after
}

/// `String.htmlDecode` (`Html.fromHtml` in legacy mode) for tracker and catalogue text: entities are decoded,
/// `<br>` and the end of `<p>`, `<div>` and `<li>` become a newline, tags are removed and their text kept, and
/// `<script>` and `<style>` content is dropped. Tags are removed before entities are decoded, so `&lt;b&gt;`
/// stays the text `<b>`.
pub fn html_decode_legacy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(['<', '&']) {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        rest = if rest.starts_with('<') {
            skip_tag(rest, &mut out)
        } else {
            push_entity(rest, &mut out)
        };
    }
    out.push_str(rest);
    out
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
    fn valid_filename_trims_a_trailing_dot_or_space_left_by_the_cut_when_ntfs() {
        let spaced = format!("{} b", "a".repeat(239));
        assert_eq!(build_valid_filename(&spaced, true), "a".repeat(239));
        assert_eq!(build_valid_filename(&spaced, false), format!("{} ", "a".repeat(239)));
        let dotted = format!("{}.x", "a".repeat(239));
        assert_eq!(build_valid_filename(&dotted, true), "a".repeat(239));
    }

    #[test]
    fn valid_filename_reserves_device_names_only_when_ntfs() {
        assert_eq!(build_valid_filename("LPT1.txt", true), "LPT1_.txt");
        assert_eq!(build_valid_filename("nul", true), "nul_");
        assert_eq!(build_valid_filename("CONSOLE", true), "CONSOLE");
        assert_eq!(build_valid_filename("CON", false), "CON");
        assert_eq!(build_valid_filename("  ..Title.. ", true), "Title");
        assert_eq!(build_valid_filename("a/b:c", true), "a_b_c");
        assert_eq!(build_valid_filename("   ", true), "(invalid)");
    }

    #[test]
    fn byte_size_counts_utf8_bytes() {
        assert_eq!(byte_size("日"), 3);
        assert_eq!(byte_size(""), 0);
        assert_eq!(byte_size("a😀"), 5);
    }

    #[test]
    fn html_decode_spec_vectors() {
        assert_eq!(html_decode_legacy("Tom &amp; Jerry"), "Tom & Jerry");
        assert_eq!(html_decode_legacy("a<br>b"), "a\nb");
        assert_eq!(html_decode_legacy("a<BR/>b<br />c"), "a\nb\nc");
        assert_eq!(html_decode_legacy("&#8217;"), "\u{2019}");
        assert_eq!(html_decode_legacy("&#x2019;"), "\u{2019}");
    }

    #[test]
    fn html_decode_removes_tags_and_keeps_their_text() {
        assert_eq!(html_decode_legacy("<b>bold</b> and <a href=\"x>y\">link</a>"), "bold and link");
        assert_eq!(html_decode_legacy("<p>one</p><p>two</p>"), "one\ntwo\n");
        assert_eq!(html_decode_legacy("<!-- note -->text"), "text");
    }

    #[test]
    fn html_decode_drops_script_and_style_text() {
        assert_eq!(html_decode_legacy("a<script>alert('<b>x</b>')</script>b"), "ab");
        assert_eq!(html_decode_legacy("a<STYLE>p{}</STYLE>b"), "ab");
        assert_eq!(html_decode_legacy("a<script>never closed"), "a");
    }

    #[test]
    fn html_decode_leaves_unknown_references_and_stray_brackets() {
        assert_eq!(html_decode_legacy("a & b"), "a & b");
        assert_eq!(html_decode_legacy("&bogus; &#0; &#xD800;"), "&bogus; &#0; &#xD800;");
        assert_eq!(html_decode_legacy("1 < 2 and 3 > 2"), "1 < 2 and 3 > 2");
        assert_eq!(html_decode_legacy("&lt;b&gt;"), "<b>");
        assert_eq!(html_decode_legacy("a<b"), "a<b");
    }

    #[test]
    fn html_decode_named_entities() {
        assert_eq!(LATIN1.len(), 96);
        assert_eq!(html_decode_legacy("caf&eacute; &nbsp;&yuml;&copy;"), "caf\u{e9} \u{a0}\u{ff}\u{a9}");
        assert_eq!(html_decode_legacy("&ldquo;hi&rdquo; &ndash; &hellip;"), "\u{201c}hi\u{201d} \u{2013} \u{2026}");
    }

    #[test]
    fn html_decode_anilist_sample_payload() {
        let title = "Frieren: Beyond Journey&#8217;s End<br>Season 2 &amp; <i>OVA</i><script>track()</script>";
        assert_eq!(
            html_decode_legacy(title),
            "Frieren: Beyond Journey\u{2019}s End\nSeason 2 & OVA"
        );
    }

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
