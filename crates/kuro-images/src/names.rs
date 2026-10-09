//! File names for saved images: the Android-compatible sanitiser, byte-limited titles and the reader and player
//! name rules (area:34 behavior section 22, design section 8.2).
//!
//! The base sanitiser is the same port `kuro-downloads` uses for its folder names. `kuro-images` cannot depend on
//! that crate (it depends on this one), so the rule is kept here as well.

/// The title budget in UTF-8 bytes for a saved page or frame name (behavior section 22.3).
pub const MAX_TITLE_BYTES: usize = 250;
/// Kotlin's `take(240)` counts UTF-16 units (behavior section 22.1).
const MAX_UTF16_UNITS: usize = 240;

/// Device names that Windows refuses as file names (design section 8.2, addendum rule 5).
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// `DiskUtil.buildValidFilename` (behavior section 22.1), then the reserved-name rule. Trims leading and trailing
/// dots and spaces, maps forbidden and control characters to `_`, returns `(invalid)` for an empty result, and
/// keeps at most 240 UTF-16 units.
pub fn build_valid_filename(orig: &str) -> String {
    let trimmed = orig.trim_matches(|c| c == '.' || c == ' ');
    if trimmed.is_empty() {
        return "(invalid)".to_owned();
    }
    let mapped: String = trimmed
        .chars()
        .map(|c| if is_forbidden(c) { '_' } else { c })
        .collect();
    reserve_device_name(take_utf16(&mapped, MAX_UTF16_UNITS).to_owned())
}

fn is_forbidden(c: char) -> bool {
    c <= '\u{1f}' || c == '\u{7f}' || "\"*/:<>?\\|".contains(c)
}

fn take_utf16(s: &str, units: usize) -> &str {
    let mut used = 0;
    for (i, c) in s.char_indices() {
        used += c.len_utf16();
        if used > units {
            return &s[..i];
        }
    }
    s
}

/// `CON.txt` becomes `CON_.txt`. The stem is compared without its extension and without trailing spaces.
fn reserve_device_name(name: String) -> String {
    let stem_end = name.find('.').unwrap_or(name.len());
    let stem = name[..stem_end].trim_end_matches(' ');
    if RESERVED.iter().any(|r| stem.eq_ignore_ascii_case(r)) {
        format!("{stem}_{}", &name[stem_end..])
    } else {
        name
    }
}

/// The first `n` UTF-8 bytes of `s`, cut on a character boundary (behavior section 22.2). A character cut in the
/// middle is dropped, which is what Kotlin's decode-and-strip does.
pub fn take_bytes(s: &str, n: usize) -> String {
    let mut end = n.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

/// UTF-8 length of `s` in bytes (`String.byteSize`).
pub fn byte_size(s: &str) -> usize {
    s.len()
}

/// Name of a saved reader page: `<title> - <chapter>` cut to fit the budget, sanitised, then ` - <page number>`
/// appended unsanitised (behavior section 22.3).
pub fn reader_file_name(title: &str, chapter_name: &str, page_number: usize) -> String {
    titled_name(
        &format!("{title} - {chapter_name}"),
        &format!(" - {page_number}"),
    )
}

/// Name of a saved player frame: `<title> - <episode>`, then ` - <HH:MM:SS>` (behavior section 22.4).
pub fn player_file_name(title: &str, episode_name: &str, time_text: &str) -> String {
    titled_name(
        &format!("{title} - {episode_name}"),
        &format!(" - {time_text}"),
    )
}

fn titled_name(head: &str, suffix: &str) -> String {
    let budget = MAX_TITLE_BYTES.saturating_sub(byte_size(suffix));
    format!(
        "{}{suffix}",
        build_valid_filename(&take_bytes(head, budget))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitiser_matches_the_kotlin_rules() {
        assert_eq!(build_valid_filename("  ..Title.. "), "Title");
        assert_eq!(build_valid_filename("   "), "(invalid)");
        assert_eq!(
            build_valid_filename("a/b:c*d?e\"f<g>h|i\\j"),
            "a_b_c_d_e_f_g_h_i_j"
        );
        assert_eq!(build_valid_filename("\u{7}"), "_");
        assert_eq!(build_valid_filename("Name (EN)"), "Name (EN)");
        assert_eq!(build_valid_filename("Mangá/Ż"), "Mangá_Ż");
    }

    #[test]
    fn sanitiser_cuts_to_240_utf16_units() {
        let emoji = "\u{1F600}".repeat(200);
        let out = build_valid_filename(&emoji);
        assert_eq!(out.encode_utf16().count(), 240);
        assert_eq!(out, "\u{1F600}".repeat(120));
    }

    #[test]
    fn reserved_device_names_get_an_underscore() {
        assert_eq!(build_valid_filename("CON"), "CON_");
        assert_eq!(build_valid_filename("nul"), "nul_");
        assert_eq!(build_valid_filename("Com9.jpg"), "Com9_.jpg");
        assert_eq!(build_valid_filename("CONSOLE"), "CONSOLE");
    }

    #[test]
    fn take_bytes_keeps_whole_characters_only() {
        assert_eq!(take_bytes("abc", 10), "abc");
        assert_eq!(take_bytes("abc", 2), "ab");
        // "é" is two bytes; cutting after the first byte drops it.
        assert_eq!(take_bytes("aé", 2), "a");
        assert_eq!(take_bytes("aé", 3), "aé");
        assert_eq!(take_bytes("", 0), "");
    }

    #[test]
    fn byte_size_counts_utf8_bytes() {
        assert_eq!(byte_size("abc"), 3);
        assert_eq!(byte_size("é"), 2);
        assert_eq!(byte_size("\u{1F600}"), 4);
    }

    #[test]
    fn reader_name_is_title_chapter_then_page() {
        assert_eq!(reader_file_name("A", "B", 3), "A - B - 3");
        assert_eq!(reader_file_name("A/A", "B:1", 12), "A_A - B_1 - 12");
    }

    #[test]
    fn reader_name_is_cut_to_the_byte_budget_before_the_suffix() {
        let long = "x".repeat(400);
        let name = reader_file_name(&long, "c", 7);
        assert!(name.ends_with(" - 7"));
        assert!(byte_size(&name) <= MAX_TITLE_BYTES);
        // The sanitiser keeps 240 UTF-16 units of the 246 bytes, then the 4-byte suffix follows.
        assert_eq!(name, format!("{}{}", "x".repeat(240), " - 7"));
    }

    #[test]
    fn reader_name_never_splits_a_character() {
        let long = "é".repeat(300);
        let name = reader_file_name(&long, "", 1);
        assert!(name.ends_with(" - 1"));
        assert!(name.is_char_boundary(name.len()));
        assert!(byte_size(&name) <= MAX_TITLE_BYTES);
        assert!(!name.contains('\u{FFFD}'));
    }

    #[test]
    fn player_name_uses_the_time_suffix() {
        assert_eq!(
            player_file_name("Show", "Episode 2", "00:01:05"),
            "Show - Episode 2 - 00:01:05"
        );
    }
}
