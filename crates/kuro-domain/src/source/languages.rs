//! Language groups of the source filter screens (`GetLanguagesWithMangaSources.kt`, `LocaleHelper.comparator`; T-17a-017).
//! The caller passes the online (HTTP) sources only, and the localized display-name comparator, so this module does no
//! locale lookup of its own (the display names are T-17a-045).

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::app::source::cmp_ignore_case_java;
use crate::source::model::Source;

/// The sources of one language, in the order the filter screen shows them. `enabled` is a property of the language, not of
/// its sources: a group whose sources are all disabled is still listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageGroup {
    pub lang: String,
    pub enabled: bool,
    pub sources: Vec<Source>,
}

/// Groups `online` by language. Inside a group a disabled source comes last, then the name ignoring case. Groups come in
/// this order: enabled languages first; then `all`; then `display_cmp` (the localized display names); then the raw code.
pub fn group_by_language(
    online: &[Source],
    enabled: &BTreeSet<String>,
    disabled: &BTreeSet<String>,
    display_cmp: &dyn Fn(&str, &str) -> Ordering,
) -> Vec<LanguageGroup> {
    let mut sorted: Vec<&Source> = online.iter().collect();
    sorted.sort_by(|a, b| {
        disabled
            .contains(&a.id.to_string())
            .cmp(&disabled.contains(&b.id.to_string()))
            .then_with(|| cmp_ignore_case_java(&a.name, &b.name))
    });

    let mut groups: Vec<LanguageGroup> = Vec::new();
    for source in sorted {
        match groups.iter_mut().find(|group| group.lang == source.lang) {
            Some(group) => group.sources.push(source.clone()),
            None => groups.push(LanguageGroup {
                lang: source.lang.clone(),
                enabled: enabled.contains(&source.lang),
                sources: vec![source.clone()],
            }),
        }
    }

    groups.sort_by(|a, b| {
        (!a.enabled)
            .cmp(&!b.enabled)
            .then_with(|| (a.lang != "all").cmp(&(b.lang != "all")))
            .then_with(|| display_cmp(&a.lang, &b.lang))
            .then_with(|| a.lang.cmp(&b.lang))
    });
    groups
}

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

/// `GetLanguagesWithMangaSources` / `GetLanguagesWithAnimeSources`: the language groups of the online sources, ordered by
/// `(not enabled)` then [`lang_cmp`]. The caller passes the online sources only.
pub fn languages_with_sources(
    online: &[Source],
    enabled: &BTreeSet<String>,
    disabled: &BTreeSet<String>,
    display: &dyn Fn(&str) -> String,
) -> Vec<LanguageGroup> {
    group_by_language(online, enabled, disabled, &|a, b| lang_cmp(a, b, display))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    /// The English display names of the codes the tests use, so the comparator is the localized order.
    fn english(code: &str) -> &'static str {
        match code {
            "all" => "All",
            "en" => "English",
            "ja" => "Japanese",
            "fr" => "French",
            _ => "Other",
        }
    }

    fn display_cmp(a: &str, b: &str) -> Ordering {
        english(a).cmp(english(b))
    }

    fn src(id: i64, name: &str, lang: &str) -> Source {
        Source::new(id, name, lang)
    }

    fn names(group: &LanguageGroup) -> Vec<&str> {
        group.sources.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn the_worked_example_gives_en_ja_fr_and_disabled_sources_come_last() {
        // A (en) enabled, B (en) disabled, C (ja) enabled, D (fr); languages {all, en, ja}; disabled {B}.
        let online = [src(1, "A", "en"), src(2, "B", "en"), src(3, "C", "ja"), src(4, "D", "fr")];
        let groups = group_by_language(&online, &set(&["all", "en", "ja"]), &set(&["2"]), &display_cmp);
        let order: Vec<&str> = groups.iter().map(|g| g.lang.as_str()).collect();
        assert_eq!(order, ["en", "ja", "fr"]);
        assert_eq!(names(&groups[0]), ["A", "B"], "B is disabled, so it comes after A");
        assert!(groups[0].enabled && groups[1].enabled && !groups[2].enabled);
    }

    #[test]
    fn a_group_whose_sources_are_all_disabled_is_still_listed() {
        let online = [src(1, "A", "en"), src(2, "B", "ja")];
        let groups = group_by_language(&online, &set(&["en", "ja"]), &set(&["1", "2"]), &display_cmp);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].sources.len(), 1);
    }

    #[test]
    fn within_a_group_the_name_is_compared_ignoring_case() {
        let online = [src(1, "beta", "en"), src(2, "Alpha", "en"), src(3, "ALPHA", "en")];
        let groups = group_by_language(&online, &set(&["en"]), &set(&[]), &display_cmp);
        assert_eq!(names(&groups[0]), ["Alpha", "ALPHA", "beta"]);
    }

    #[test]
    fn all_comes_first_among_the_groups_that_are_not_enabled() {
        let online = [src(1, "A", "fr"), src(2, "B", "all"), src(3, "C", "ja")];
        let groups = group_by_language(&online, &set(&["en"]), &set(&[]), &display_cmp);
        let order: Vec<&str> = groups.iter().map(|g| g.lang.as_str()).collect();
        assert_eq!(order, ["all", "fr", "ja"], "none enabled: all first, then by display name (French before Japanese)");
    }

    #[test]
    fn a_tie_in_the_display_name_is_broken_by_the_raw_code() {
        let online = [src(1, "A", "xx"), src(2, "B", "yy")];
        let same_name = |_: &str, _: &str| Ordering::Equal;
        let groups = group_by_language(&online, &set(&[]), &set(&[]), &same_name);
        let order: Vec<&str> = groups.iter().map(|g| g.lang.as_str()).collect();
        assert_eq!(order, ["xx", "yy"]);
    }

    #[test]
    fn an_empty_online_list_gives_no_groups() {
        assert!(group_by_language(&[], &set(&["en"]), &set(&[]), &display_cmp).is_empty());
    }

    /// `sources::language_order`: enabled block first, `all` first inside it, then display names by UTF-16 order; `zh-CN`
    /// and `zh-TW` look up the `zh-Hans` and `zh-Hant` names.
    #[test]
    fn language_order_follows_the_enabled_block_then_the_display_names() {
        let names = |tag: &str| match tag {
            "all" => "Multi".to_owned(),
            "en" => "English".to_owned(),
            "ja" => "日本語".to_owned(),
            "fr" => "Français".to_owned(),
            "zh-Hans" => "中文（简体）".to_owned(),
            "zh-Hant" => "中文（繁體）".to_owned(),
            other => other.to_owned(),
        };
        let online = [
            src(1, "F", "fr"),
            src(2, "Z1", "zh-CN"),
            src(3, "Z2", "zh-TW"),
            src(4, "J", "ja"),
            src(5, "E", "en"),
            src(6, "M", "all"),
        ];
        let groups = languages_with_sources(&online, &set(&["all", "en", "ja"]), &set(&[]), &names);
        let order: Vec<&str> = groups.iter().map(|g| g.lang.as_str()).collect();
        assert_eq!(order, ["all", "en", "ja", "fr", "zh-CN", "zh-TW"]);
        assert_eq!(display_tag("zh-CN"), "zh-Hans");
    }

    #[test]
    fn utf16_order_puts_uppercase_before_lowercase_and_non_ascii_last() {
        assert_eq!(utf16_cmp("Zebra", "apple"), Ordering::Less, "Java String.compareTo, not a collation");
        assert_eq!(utf16_cmp("é", "z"), Ordering::Greater);
    }
}
