//! Display names, language ordering and string comparison for the browse lists (T-40a-033, T-40b-002).

use std::cmp::Ordering;

use kuro_domain::source::languages::{display_tag, lang_cmp};
use kuro_i18n::source_names::display_name;


/// The label of a source-list key: the special groups (`last_used`, `pinned`, `other`, `all`) or a language's own name.
pub fn source_display_name(key: &str) -> String {
    display_name(key)
}

/// `LocaleHelper.getLocalizedDisplayName`: `zh-CN` is looked up as `zh-Hans` and `zh-TW` as `zh-Hant`.
pub fn localized_display_name(lang: &str) -> String {
    display_name(display_tag(lang))
}

/// `LocaleHelper.getLocalizedDisplayName` as the global search header uses it for its subtitle (E19): the language's own
/// name, and for the `all` tag the tag itself with its first letter capitalised, which is what Java answers for it. The
/// source lists use `localized_display_name`, where `all` is the Multi label.
pub fn subtitle_display_name(lang: &str) -> String {
    if lang == "all" {
        return "All".to_owned();
    }
    localized_display_name(lang)
}

/// `LocaleHelper.comparator` as a total order. `all` sorts first, then languages by their localized names. Two tags
/// with one name stay apart by tag (E-40-14), and equal tags compare equal, so `all` is reflexive (TC-40-010).
pub fn compare_languages(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    lang_cmp(a, b, &localized_display_name).then_with(|| a.encode_utf16().cmp(b.encode_utf16()))
}

/// Case-insensitive order for source and extension names (`String.CASE_INSENSITIVE_ORDER`). A total preorder: strings
/// that differ only by case compare equal.
pub fn case_insensitive_cmp(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_core::text::primary_cmp;

    #[test]
    fn special_keys_have_their_four_labels() {
        assert_eq!(source_display_name("last_used"), "Last used");
        assert_eq!(source_display_name("pinned"), "Pinned");
        assert_eq!(source_display_name("other"), "Other");
        assert_eq!(source_display_name("all"), "Multi");
    }

    #[test]
    fn zh_cn_and_zh_tw_use_the_hans_and_hant_names() {
        assert_eq!(localized_display_name("zh-CN"), localized_display_name("zh-Hans"));
        assert_eq!(localized_display_name("zh-TW"), localized_display_name("zh-Hant"));
        assert_ne!(localized_display_name("zh-CN"), "zh-CN");
    }

    #[test]
    fn all_sorts_first_and_the_order_is_reflexive_and_total() {
        assert_eq!(compare_languages("all", "all"), Ordering::Equal);
        assert_eq!(compare_languages("all", "en"), Ordering::Less);
        assert_eq!(compare_languages("de", "all"), Ordering::Greater);
        let tags = ["all", "en", "de", "ja", "", "zh-CN", "zh-Hans", "fr"];
        for a in tags {
            for b in tags {
                assert_eq!(compare_languages(a, b), compare_languages(b, a).reverse(), "{a} vs {b}");
                for c in tags {
                    if compare_languages(a, b) != Ordering::Greater && compare_languages(b, c) != Ordering::Greater {
                        assert_ne!(compare_languages(a, c), Ordering::Greater, "{a} {b} {c}");
                    }
                }
            }
        }
    }

    #[test]
    fn primary_strength_ignores_case_and_accents() {
        assert_eq!(primary_cmp("é", "e"), Ordering::Equal);
        assert_eq!(primary_cmp("A", "a"), Ordering::Equal);
        assert_eq!(primary_cmp("a", "b"), Ordering::Less);
    }

    #[test]
    fn case_insensitive_order_is_a_total_preorder_on_ascii() {
        assert_eq!(case_insensitive_cmp("Alpha", "alpha"), Ordering::Equal);
        assert_eq!(case_insensitive_cmp("alpha", "Beta"), Ordering::Less);
        assert_eq!(case_insensitive_cmp("Beta", "alpha"), Ordering::Greater);
    }

    #[test]
    fn the_global_search_subtitle_shows_all_as_all_and_not_as_multi() {
        assert_eq!(subtitle_display_name("all"), "All");
        assert_eq!(source_display_name("all"), "Multi", "the source lists keep their Multi label");
        assert_eq!(subtitle_display_name("zh-CN"), localized_display_name("zh-Hans"));
        assert_eq!(subtitle_display_name("en"), localized_display_name("en"));
    }
}
