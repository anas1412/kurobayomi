//! Name sanitiser: port of `DiskUtil.buildValidFilename` (behaviour 8.3) and the Windows reserved-name rule (addendum
//! rule 5, decision D-DL-01).

/// Makes one path component with the shared sanitiser, `kuro_core::text::build_valid_filename`. With the
/// `ntfs-names` feature (default) the NTFS rules apply (trailing dot or space after the cut, reserved device names);
/// without it the output equals the Kotlin output.
pub fn build_valid_filename(orig: &str) -> String {
    kuro_core::text::build_valid_filename(orig, cfg!(feature = "ntfs-names"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // TST-34
    #[test]
    fn edge_case_table_ec_71_to_ec_79() {
        // EC-71: dots and spaces at both ends.
        assert_eq!(build_valid_filename("  ..Title.. "), "Title");
        // EC-72: nothing left after trimming.
        assert_eq!(build_valid_filename("   "), "(invalid)");
        // EC-73: every forbidden character.
        assert_eq!(build_valid_filename("a/b:c*d?e\"f<g>h|i\\j"), "a_b_c_d_e_f_g_h_i_j");
        // EC-74: a control character.
        assert_eq!(build_valid_filename("\u{7}"), "_");
        // EC-75: 200 emoji, two UTF-16 units each, cut at 240 units.
        let emoji = "\u{1F600}".repeat(200);
        let out = build_valid_filename(&emoji);
        assert_eq!(out.encode_utf16().count(), 240);
        assert_eq!(out, "\u{1F600}".repeat(120));
        // EC-76: reserved device name.
        assert_eq!(build_valid_filename("CON"), "CON_");
        // EC-77: parentheses are allowed.
        assert_eq!(build_valid_filename("Name (EN)"), "Name (EN)");
        // EC-78: non-ASCII letters are kept.
        assert_eq!(build_valid_filename("Mangá/Ż"), "Mangá_Ż");
        // EC-79: a colon and an underscore produce the same folder (documented collision).
        assert_eq!(build_valid_filename("a:b"), build_valid_filename("a_b"));
    }

    #[test]
    fn reserved_names_match_with_any_case_and_extension() {
        assert_eq!(build_valid_filename("nul"), "nul_");
        assert_eq!(build_valid_filename("Com9"), "Com9_");
        assert_eq!(build_valid_filename("LPT1.txt"), "LPT1_.txt");
        assert_eq!(build_valid_filename("CONSOLE"), "CONSOLE");
        assert_eq!(build_valid_filename("COM10"), "COM10");
    }

    #[test]
    fn names_without_reserved_words_match_the_kotlin_rule() {
        assert_eq!(build_valid_filename("One Piece: Chapter 1?"), "One Piece_ Chapter 1_");
        assert_eq!(build_valid_filename("...  "), "(invalid)");
        assert_eq!(build_valid_filename("v1.2 final."), "v1.2 final");
    }
}
