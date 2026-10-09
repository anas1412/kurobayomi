//! Source names as the UI shows them: the display name (`HttpSource.toString`, T-16a-002, T-15a-044) and the name
//! the library uses for a manga from a source (`getNameForMangaInfo`, `MangaSourceExtensions.kt`). The enabled
//! languages are read from the `source_languages` preference of `kuro-prefs` (default `all`, `en`, system language).

/// `"{name} ({LANG})"` with the language uppercased, as `HttpSource.toString()` builds it.
pub fn display_name(name: &str, lang: &str) -> String {
    format!("{name} ({})", lang.to_uppercase())
}

/// The bare name when exactly one language is enabled (after dropping `all` and `other`) and the source's language is
/// that one. Otherwise, and when the source's language is not enabled, the display name. `enabled_languages` is the
/// `source_languages` preference.
pub fn name_for_manga_info(name: &str, lang: &str, enabled_languages: &[String]) -> String {
    let active: Vec<&str> =
        enabled_languages.iter().map(String::as_str).filter(|l| !matches!(*l, "all" | "other")).collect();
    match active.as_slice() {
        [only] if *only == lang => name.to_owned(),
        _ => display_name(name, lang),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn the_display_name_uppercases_the_language() {
        assert_eq!(display_name("Example", "en"), "Example (EN)");
        assert_eq!(display_name("Aniyomi Test", "ja"), "Aniyomi Test (JA)");
    }

    #[test]
    fn one_enabled_language_that_matches_hides_the_tag() {
        assert_eq!(name_for_manga_info("Example", "en", &langs(&["en"])), "Example");
        assert_eq!(name_for_manga_info("Example", "en", &langs(&["all", "en"])), "Example");
    }

    #[test]
    fn several_enabled_languages_show_the_tag() {
        assert_eq!(name_for_manga_info("Example", "en", &langs(&["en", "fr"])), "Example (EN)");
    }

    #[test]
    fn a_source_whose_language_is_not_enabled_shows_the_tag() {
        assert_eq!(name_for_manga_info("Example", "ja", &langs(&["en"])), "Example (JA)");
        assert_eq!(name_for_manga_info("Example", "en", &langs(&["all"])), "Example (EN)");
    }

    #[test]
    fn zero_enabled_languages_always_shows_the_tag() {
        assert_eq!(name_for_manga_info("Example", "en", &langs(&[])), "Example (EN)");
    }

    #[test]
    fn the_excluded_tags_all_and_other_do_not_count_as_enabled_languages() {
        assert_eq!(name_for_manga_info("Example", "en", &langs(&["other", "en"])), "Example", "one active language");
        assert_eq!(name_for_manga_info("Example", "en", &langs(&["all", "other"])), "Example (EN)", "none active");
        assert_eq!(name_for_manga_info("Example", "en", &langs(&["other", "all", "en", "ja"])), "Example (EN)");
    }
}
