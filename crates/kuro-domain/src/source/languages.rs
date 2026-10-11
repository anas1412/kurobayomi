//! The language order of the source filter screens (`LocaleHelper.comparator`; T-17a-017): the display tag of a language and
//! the comparator over the localized display names. The screens group their sources in `browse::lang_pages`, which uses
//! these two functions through `browse::names`.

use std::cmp::Ordering;

/// `LocaleHelper.getLocalizedDisplayName` looks up `zh-CN` as `zh-Hans` and `zh-TW` as `zh-Hant`. Every other tag is
/// looked up as it is.
pub fn display_tag(lang: &str) -> &str {
    match lang {
        "zh-CN" => "zh-Hans",
        "zh-TW" => "zh-Hant",
        other => other,
    }
}

/// `String.compareTo` in Java: order by UTF-16 code units, case-sensitive. Not a collation.
pub fn utf16_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `LocaleHelper.comparator`: `all` sorts first; otherwise the localized display names are compared by UTF-16 code units.
/// `display` maps a display tag (see [`display_tag`]) to the language's own name, as `lang_display_name` does.
pub fn lang_cmp(a: &str, b: &str, display: &dyn Fn(&str) -> String) -> Ordering {
    match (a == "all", b == "all") {
        (true, _) => Ordering::Less,
        (_, true) => Ordering::Greater,
        _ => utf16_cmp(&display(display_tag(a)), &display(display_tag(b))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_order_puts_uppercase_before_lowercase_and_non_ascii_last() {
        assert_eq!(utf16_cmp("Zebra", "apple"), Ordering::Less, "Java String.compareTo, not a collation");
        assert_eq!(utf16_cmp("é", "z"), Ordering::Greater);
    }

    #[test]
    fn chinese_regions_look_up_their_script_names() {
        assert_eq!(display_tag("zh-CN"), "zh-Hans");
        assert_eq!(display_tag("zh-TW"), "zh-Hant");
        assert_eq!(display_tag("ja"), "ja");
    }

    #[test]
    fn all_sorts_first_and_the_other_languages_by_their_own_names() {
        let names = |tag: &str| match tag {
            "fr" => "Français".to_owned(),
            "ja" => "日本語".to_owned(),
            other => other.to_owned(),
        };
        assert_eq!(lang_cmp("all", "fr", &names), Ordering::Less);
        assert_eq!(lang_cmp("fr", "all", &names), Ordering::Greater);
        assert_eq!(lang_cmp("fr", "ja", &names), Ordering::Less, "Français before 日本語 by UTF-16 code units");
    }
}
