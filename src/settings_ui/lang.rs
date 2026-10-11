//! Language list validator (task T-48a-030, edge cases E-LG-01..07). Used by the subtitle and audio preferred
//! language fields. Codes are checked against a static table: every ISO 639-1 code, its ISO 639-2 three-letter
//! forms (terminology and bibliographic), and a few common ISO 639-3 codes (decision D-03: both lengths are accepted).
// Not reachable from the player pages yet: the subtitle and audio pages are built by `pref_model/pages.rs`.
use std::fmt;

/// ISO 639-1 code and its ISO 639-2/T code.
const LANGUAGES: &[(&str, &str)] = &[
    ("aa", "aar"), ("ab", "abk"), ("ae", "ave"), ("af", "afr"), ("ak", "aka"), ("am", "amh"), ("an", "arg"),
    ("ar", "ara"), ("as", "asm"), ("av", "ava"), ("ay", "aym"), ("az", "aze"), ("ba", "bak"), ("be", "bel"),
    ("bg", "bul"), ("bh", "bih"), ("bi", "bis"), ("bm", "bam"), ("bn", "ben"), ("bo", "bod"), ("br", "bre"),
    ("bs", "bos"), ("ca", "cat"), ("ce", "che"), ("ch", "cha"), ("co", "cos"), ("cr", "cre"), ("cs", "ces"),
    ("cu", "chu"), ("cv", "chv"), ("cy", "cym"), ("da", "dan"), ("de", "deu"), ("dv", "div"), ("dz", "dzo"),
    ("ee", "ewe"), ("el", "ell"), ("en", "eng"), ("eo", "epo"), ("es", "spa"), ("et", "est"), ("eu", "eus"),
    ("fa", "fas"), ("ff", "ful"), ("fi", "fin"), ("fj", "fij"), ("fo", "fao"), ("fr", "fra"), ("fy", "fry"),
    ("ga", "gle"), ("gd", "gla"), ("gl", "glg"), ("gn", "grn"), ("gu", "guj"), ("gv", "glv"), ("ha", "hau"),
    ("he", "heb"), ("hi", "hin"), ("ho", "hmo"), ("hr", "hrv"), ("ht", "hat"), ("hu", "hun"), ("hy", "hye"),
    ("hz", "her"), ("ia", "ina"), ("id", "ind"), ("ie", "ile"), ("ig", "ibo"), ("ii", "iii"), ("ik", "ipk"),
    ("io", "ido"), ("is", "isl"), ("it", "ita"), ("iu", "iku"), ("ja", "jpn"), ("jv", "jav"), ("ka", "kat"),
    ("kg", "kon"), ("ki", "kik"), ("kj", "kua"), ("kk", "kaz"), ("kl", "kal"), ("km", "khm"), ("kn", "kan"),
    ("ko", "kor"), ("kr", "kau"), ("ks", "kas"), ("ku", "kur"), ("kv", "kom"), ("kw", "cor"), ("ky", "kir"),
    ("la", "lat"), ("lb", "ltz"), ("lg", "lug"), ("li", "lim"), ("ln", "lin"), ("lo", "lao"), ("lt", "lit"),
    ("lu", "lub"), ("lv", "lav"), ("mg", "mlg"), ("mh", "mah"), ("mi", "mri"), ("mk", "mkd"), ("ml", "mal"),
    ("mn", "mon"), ("mr", "mar"), ("ms", "msa"), ("mt", "mlt"), ("my", "mya"), ("na", "nau"), ("nb", "nob"),
    ("nd", "nde"), ("ne", "nep"), ("ng", "ndo"), ("nl", "nld"), ("nn", "nno"), ("no", "nor"), ("nr", "nbl"),
    ("nv", "nav"), ("ny", "nya"), ("oc", "oci"), ("oj", "oji"), ("om", "orm"), ("or", "ori"), ("os", "oss"),
    ("pa", "pan"), ("pi", "pli"), ("pl", "pol"), ("ps", "pus"), ("pt", "por"), ("qu", "que"), ("rm", "roh"),
    ("rn", "run"), ("ro", "ron"), ("ru", "rus"), ("rw", "kin"), ("sa", "san"), ("sc", "srd"), ("sd", "snd"),
    ("se", "sme"), ("sg", "sag"), ("si", "sin"), ("sk", "slk"), ("sl", "slv"), ("sm", "smo"), ("sn", "sna"),
    ("so", "som"), ("sq", "sqi"), ("sr", "srp"), ("ss", "ssw"), ("st", "sot"), ("su", "sun"), ("sv", "swe"),
    ("sw", "swa"), ("ta", "tam"), ("te", "tel"), ("tg", "tgk"), ("th", "tha"), ("ti", "tir"), ("tk", "tuk"),
    ("tl", "tgl"), ("tn", "tsn"), ("to", "ton"), ("tr", "tur"), ("ts", "tso"), ("tt", "tat"), ("tw", "twi"),
    ("ty", "tah"), ("ug", "uig"), ("uk", "ukr"), ("ur", "urd"), ("uz", "uzb"), ("ve", "ven"), ("vi", "vie"),
    ("vo", "vol"), ("wa", "wln"), ("wo", "wol"), ("xh", "xho"), ("yi", "yid"), ("yo", "yor"), ("za", "zha"),
    ("zh", "zho"), ("zu", "zul"),
];

/// ISO 639-2/B codes that differ from the terminology code.
const BIBLIOGRAPHIC: &[&str] = &[
    "alb", "arm", "baq", "bur", "chi", "cze", "dut", "fre", "geo", "ger", "gre", "ice", "mac", "mao", "may", "per",
    "rum", "slo", "tib", "wel",
];

/// Common ISO 639-3 codes that have no ISO 639-1 code.
const EXTRA_THREE: &[&str] = &["ast", "ceb", "ckb", "cmn", "fil", "hak", "haw", "hmn", "kmr", "lad", "nan", "nds", "sco", "wuu", "yue"];

/// The first element of a list that is not a language code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LangError {
    InvalidCode(String),
}

impl fmt::Display for LangError {
    /// `pref_player_subtitle_invalid_lang`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LangError::InvalidCode(code) => write!(f, "Invalid code: \"{code}\""),
        }
    }
}

/// True when `code` (any case) is a two- or three-letter code of the table.
pub fn is_language_code(code: &str) -> bool {
    let code = code.to_ascii_lowercase();
    match code.len() {
        2 => LANGUAGES.iter().any(|(two, _)| *two == code),
        3 => LANGUAGES.iter().any(|(_, three)| *three == code) || BIBLIOGRAPHIC.contains(&code.as_str()) || EXTRA_THREE.contains(&code.as_str()),
        _ => false,
    }
}

/// Splits on `,`, drops empty elements, trims, then validates each element in order (Kotlin order: `en, ` fails on
/// the blank left after the trim, E-LG-03). The error names the first bad element.
pub fn validate_list(text: &str) -> Result<(), LangError> {
    for code in text.split(',').filter(|part| !part.is_empty()).map(str::trim) {
        if !is_language_code(code) {
            return Err(LangError::InvalidCode(code.to_owned()));
        }
    }
    Ok(())
}

/// [`validate_list`] as the text-field validator (`rows::edit_text`): the error is the message shown under the field.
pub fn validate_text(text: &str) -> Result<(), String> {
    validate_list(text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bad(code: &str) -> Result<(), LangError> {
        Err(LangError::InvalidCode(code.to_owned()))
    }

    #[test]
    fn e_lg_01_empty_string_is_valid() {
        assert_eq!(validate_list(""), Ok(()));
    }

    #[test]
    fn e_lg_02_trailing_comma_is_valid() {
        assert_eq!(validate_list("en,"), Ok(()));
        assert_eq!(validate_list(",,en,,ja"), Ok(()));
    }

    #[test]
    fn e_lg_03_space_after_comma_is_invalid() {
        assert_eq!(validate_list("en, "), bad(""));
    }

    #[test]
    fn e_lg_03_a_space_before_a_code_is_trimmed() {
        assert_eq!(validate_list("en, ja"), Ok(()));
    }

    #[test]
    fn e_lg_04_uppercase_is_accepted() {
        assert_eq!(validate_list("EN"), Ok(()));
        assert_eq!(validate_list("En,JPN"), Ok(()));
    }

    #[test]
    fn e_lg_05_unknown_two_letter_code_is_invalid() {
        assert_eq!(validate_list("xx"), bad("xx"));
        assert_eq!(validate_list("e"), bad("e"));
        assert_eq!(validate_list("english"), bad("english"));
    }

    #[test]
    fn e_lg_06_three_letter_codes_are_accepted() {
        for code in ["eng", "jpn", "fre", "deu", "fil", "yue"] {
            assert_eq!(validate_list(code), Ok(()), "{code}");
        }
        assert_eq!(validate_list("zzz"), bad("zzz"));
    }

    #[test]
    fn e_lg_07_the_error_names_the_first_bad_element() {
        assert_eq!(validate_list("en,xx,yy"), bad("xx"));
        assert_eq!(validate_text("en,xx,yy"), Err("Invalid code: \"xx\"".to_owned()));
        assert_eq!(validate_text("en,ja"), Ok(()));
    }

    #[test]
    fn the_table_holds_every_iso_639_1_code_once_with_a_distinct_three_letter_form() {
        assert_eq!(LANGUAGES.len(), 184);
        let mut twos: Vec<_> = LANGUAGES.iter().map(|l| l.0).collect();
        twos.sort_unstable();
        twos.dedup();
        assert_eq!(twos.len(), 184);
        assert!(LANGUAGES.iter().all(|(a, b)| a.len() == 2 && b.len() == 3));
        let mut threes: Vec<_> = LANGUAGES.iter().map(|l| l.1).collect();
        threes.sort_unstable();
        threes.dedup();
        assert_eq!(threes.len(), 184);
    }
}
