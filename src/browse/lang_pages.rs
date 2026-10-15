//! The languages of the installed sources, for the Language section of the Browse tab's view-options menu (T-40a-009).
//! A language switched off hides its sources; there are no per-source hide or pin toggles. Ports the grouping of
//! `MangaSourcesFilterScreen`; the menu itself is the shared `widgets::view_options`.

use std::collections::BTreeSet;

use kuro_domain::source::model::Source;

use super::names::{case_insensitive_cmp, compare_languages, localized_display_name};

/// One source of a language group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCheck {
    pub id: i64,
    pub name: String,
}

/// One language and the sources under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LangGroup {
    pub lang: String,
    pub title: String,
    /// The switch: the language is in `source_languages`.
    pub enabled: bool,
    pub sources: Vec<SourceCheck>,
}

/// `GetLanguagesWithMangaSources`: sources sorted by name ignoring case, grouped by language, the groups sorted by
/// `(language off, language order)`. Every source of a language is listed; the language switch decides whether it shows.
pub fn sources_filter_groups(sources: &[Source], enabled: &BTreeSet<String>) -> Vec<LangGroup> {
    // Only online sources are listed (E-SF-04): a stub and the local source are not HTTP sources.
    let mut sorted: Vec<&Source> = sources.iter().filter(|s| !s.is_stub && s.id != super::sources::LOCAL_SOURCE_ID).collect();
    sorted.sort_by(|a, b| case_insensitive_cmp(&a.name, &b.name));
    let mut groups: Vec<LangGroup> = Vec::new();
    for source in sorted {
        let check = SourceCheck { id: source.id, name: source.name.clone() };
        match groups.iter_mut().find(|g| g.lang == source.lang) {
            Some(group) => group.sources.push(check),
            None => groups.push(LangGroup {
                lang: source.lang.clone(),
                title: localized_display_name(&source.lang),
                enabled: enabled.contains(&source.lang),
                sources: vec![check],
            }),
        }
    }
    groups.sort_by(|a, b| (!a.enabled).cmp(&(!b.enabled)).then_with(|| compare_languages(&a.lang, &b.lang)));
    groups
}

#[cfg(test)]
mod pure_tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn groups_put_enabled_languages_first_and_sources_by_name_in_each() {
        let sources = vec![
            Source::new(1, "Zeta", "en"),
            Source::new(2, "alpha", "en"),
            Source::new(3, "Mid", "fr"),
            Source::new(4, "Other", "de"),
        ];
        let groups = sources_filter_groups(&sources, &set(&["en", "fr"]));
        let langs: Vec<&str> = groups.iter().map(|g| g.lang.as_str()).collect();
        assert_eq!(langs, ["en", "fr", "de"], "the off language goes last");
        assert_eq!(groups[0].sources.iter().map(|s| s.id).collect::<Vec<_>>(), [2, 1], "names ignore case");
        assert!(groups[0].enabled && !groups[2].enabled);
    }

    #[test]
    fn no_sources_means_no_groups() {
        assert!(sources_filter_groups(&[], &set(&["en"])).is_empty());
    }
}
