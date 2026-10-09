//! One file-name sanitiser for download and local-source folders and files (T-req-platform-008). The result is valid on
//! ext4 and NTFS, so a download folder written on one system reads back on the other.
//!
//! This follows Aniyomi's `DiskUtil.buildValidFilename`: trim `.` and space at both ends, `(invalid)` for an empty
//! name, forbidden characters become `_`, and the name is cut to 240 UTF-16 units. The additions are the NTFS reserved
//! device names (`CON`, `NUL`, `COM1` and so on, with or without an extension) and a trailing `.` or space left by the
//! cut. Names Aniyomi already writes unchanged come out unchanged here.

/// Cut length in UTF-16 units. Aniyomi's value; it leaves room under the 255-unit NTFS limit for a page or chapter suffix.
pub const MAX_UNITS: usize = 240;

/// The name Aniyomi uses when nothing is left after trimming.
pub const EMPTY_NAME: &str = "(invalid)";

/// Returns a file or folder name that NTFS and ext4 both accept. See the module docs for the rules.
pub fn valid_filename(orig: &str) -> String {
    let trimmed = orig.trim_matches(['.', ' ']);
    if trimmed.is_empty() {
        return EMPTY_NAME.to_owned();
    }
    let replaced: String = trimmed.chars().map(|c| if is_forbidden(c) { '_' } else { c }).collect();
    let name = if is_reserved(&replaced) { format!("_{replaced}") } else { replaced };
    let cut = truncate_units(&name, MAX_UNITS);
    let cut = cut.trim_end_matches(['.', ' ']);
    if cut.is_empty() {
        EMPTY_NAME.to_owned()
    } else {
        cut.to_owned()
    }
}

/// Control characters, DEL, and the characters NTFS and FAT forbid in a name.
fn is_forbidden(c: char) -> bool {
    c <= '\u{1f}' || c == '\u{7f}' || matches!(c, '"' | '*' | '/' | ':' | '<' | '>' | '?' | '\\' | '|')
}

/// `CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9` and `LPT1`-`LPT9`, compared without case. The device name is the part before
/// the first dot, so `con.txt` is reserved too.
fn is_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

/// The longest prefix of `s` whose UTF-16 length is at most `max`. A character is never split, so a surrogate pair stays whole.
fn truncate_units(s: &str, max: usize) -> &str {
    let mut units = 0;
    for (index, c) in s.char_indices() {
        units += c.len_utf16();
        if units > max {
            return &s[..index];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &str) -> usize {
        s.encode_utf16().count()
    }

    #[test]
    fn forbidden_characters_become_underscores() {
        assert_eq!(valid_filename("a/b\\c:d*e?f\"g<h>i|j"), "a_b_c_d_e_f_g_h_i_j");
        assert_eq!(valid_filename("tab\there\u{7f}"), "tab_here_");
    }

    #[test]
    fn trailing_dots_and_spaces_are_trimmed() {
        assert_eq!(valid_filename("Naruto. "), "Naruto");
        assert_eq!(valid_filename(" . Naruto . "), "Naruto");
    }

    #[test]
    fn an_empty_name_after_trimming_is_invalid() {
        assert_eq!(valid_filename(""), "(invalid)");
        assert_eq!(valid_filename("...  "), "(invalid)");
    }

    #[test]
    fn reserved_device_names_get_a_prefix_with_or_without_an_extension() {
        for name in ["CON", "con", "Prn.txt", "NUL", "aux.cbz", "COM1", "lpt9", "COM9.jpg"] {
            assert_eq!(valid_filename(name), format!("_{name}"), "{name}");
        }
    }

    #[test]
    fn names_that_only_start_like_a_reserved_name_are_kept() {
        for name in ["CONSOLE", "COM10", "LPT0", "nul-chapter", "Connor"] {
            assert_eq!(valid_filename(name), name, "{name}");
        }
    }

    #[test]
    fn names_are_cut_to_240_utf16_units() {
        let long = "a".repeat(300);
        assert_eq!(units(&valid_filename(&long)), MAX_UNITS);
    }

    #[test]
    fn a_surrogate_pair_is_never_split_by_the_cut() {
        let name = format!("{}\u{1F600}", "a".repeat(239));
        let cut = valid_filename(&name);
        assert_eq!(cut, "a".repeat(239));
        assert!(units(&cut) <= MAX_UNITS);
    }

    #[test]
    fn a_cut_that_ends_on_a_space_is_trimmed_again() {
        let name = format!("{} b", "a".repeat(239));
        assert_eq!(valid_filename(&name), "a".repeat(239));
    }

    #[test]
    fn output_is_stable_when_sanitised_twice() {
        for name in ["Re:Zero", "CON.", "x".repeat(260).as_str(), "  ...  "] {
            let once = valid_filename(name);
            assert_eq!(valid_filename(&once), once, "{name}");
        }
    }

    /// Names Aniyomi's `buildValidFilename` writes out. Expected values follow that function line by line, so a name
    /// Aniyomi already writes lands in the same folder here.
    #[test]
    fn matches_aniyomi_folder_naming() {
        let cases = [
            ("Vol. 1 - Ch 5", "Vol. 1 - Ch 5"),
            ("One Piece (2024)", "One Piece (2024)"),
            ("Re:Zero", "Re_Zero"),
            ("Dr. STONE?", "Dr. STONE_"),
            ("Title.", "Title"),
            ("Episode 01 [1080p] ", "Episode 01 [1080p]"),
            ("こんにちは", "こんにちは"),
        ];
        for (aniyomi_input, expected) in cases {
            assert_eq!(valid_filename(aniyomi_input), expected, "{aniyomi_input}");
        }
    }
}
