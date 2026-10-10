//! The rules behind the extension details page (T-40a-021, T-40a-022): the order of the source rows, enabling and
//! disabling every source of one extension, the base urls whose cookies are cleared, the repository link and the debug
//! text of the header. Pure: `ext_details_page` draws them.

use std::collections::BTreeSet;

/// One source row of the details page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRow {
    pub id: i64,
    /// The source's own name.
    pub name: String,
    /// The name the user sees, which may be localised.
    pub display_name: String,
    /// Whether the label of the source is its name, so the raw name sorts it (E-40-39).
    pub label_as_name: bool,
    pub enabled: bool,
}

impl SourceRow {
    /// The label the rows sort by: the raw name when it is the label, else the lowercased display name (E-40-39).
    pub(super) fn sort_label(&self) -> String {
        if self.label_as_name { self.name.clone() } else { self.display_name.to_lowercase() }
    }
}

/// Sorts the rows by `(!enabled, label)`: enabled sources first, each group by its label (B§7.1, E-40-39).
pub fn sort_rows(rows: &mut [SourceRow]) {
    rows.sort_by_cached_key(|row| (!row.enabled, row.sort_label()));
}

/// `labelAsName` of an extension's sources (behavior 4.1): a source is labelled by its own name when the extension has more
/// than one source and those sources do not all share one name (`multiSource && !multiLangSingleSource`). `names` are the
/// source names of one extension.
pub fn label_as_name(names: &[&str]) -> bool {
    let multi_source = names.len() > 1;
    let multi_lang_single_source = multi_source && names.iter().collect::<BTreeSet<_>>().len() == 1;
    multi_source && !multi_lang_single_source
}

/// Enables (removes from the disabled set) or disables every source id of the extension (E-40-38). Ids are stored as
/// strings to match upstream backups (D§5).
pub fn set_all_enabled(disabled: &BTreeSet<String>, ids: &[i64], enable: bool) -> BTreeSet<String> {
    let mut next = disabled.clone();
    for id in ids {
        if enable {
            next.remove(&id.to_string());
        } else {
            next.insert(id.to_string());
        }
    }
    next
}

/// The base urls whose cookies are cleared: each distinct http or https url once, in the order given (E-40-36).
pub fn cookie_clear_targets(base_urls: &[&str]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    base_urls
        .iter()
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
        .filter(|url| seen.insert(**url))
        .map(|url| (*url).to_owned())
        .collect()
}

/// The facts the details page shows about one installed extension (`MangaExtension.Installed`).
#[derive(Clone, Debug, PartialEq)]
pub struct ExtDetails {
    pub name: String,
    pub pkg: String,
    pub lang: String,
    pub version_name: String,
    /// The extension library version of a manga package; an anime package has none.
    pub lib_version: Option<f64>,
    pub version_code: i64,
    pub nsfw: bool,
    pub has_update: bool,
    pub obsolete: bool,
    pub shared: bool,
    pub repo_url: Option<String>,
    /// The package or image file the extension ships; the header shows it at 112 px.
    pub icon_file: Option<String>,
    pub sources: Vec<DetailSource>,
}

/// One source of the extension, before the page knows which are enabled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailSource {
    pub id: i64,
    pub name: String,
    pub lang: String,
    /// The source's label is its name (`labelAsName`), so the row title is the name and not the language.
    pub label_as_name: bool,
    /// The source has preferences of its own (`ConfigurableSource`): the row shows a settings button.
    pub configurable: bool,
    /// Set only for an HTTP source with a base url; its cookies are cleared with the others.
    pub base_url: Option<String>,
}

/// A source row ready to draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailRow {
    pub row: SourceRow,
    pub configurable: bool,
}

impl DetailRow {
    /// The title: the source's name when it is the label, else the language's display name (B§7.2).
    pub fn title(&self) -> String {
        if self.row.label_as_name { self.row.name.clone() } else { self.row.display_name.clone() }
    }
}

/// The rows of the page: enabled means not in the hidden set, then `sort_rows`.
pub fn detail_rows(sources: &[DetailSource], disabled: &BTreeSet<String>) -> Vec<DetailRow> {
    let mut rows: Vec<DetailRow> = sources
        .iter()
        .map(|s| DetailRow {
            row: SourceRow {
                id: s.id,
                name: s.name.clone(),
                display_name: super::names::localized_display_name(&s.lang),
                label_as_name: s.label_as_name,
                enabled: !disabled.contains(&s.id.to_string()),
            },
            configurable: s.configurable,
        })
        .collect();
    rows.sort_by_cached_key(|r| (!r.row.enabled, r.row.sort_label()));
    rows
}

/// `https://raw.githubusercontent.com/(.+?)/(.+?)/.+` found anywhere in the url becomes `https://github.com/$user/$repo`
/// (E-40-32). Any other url stays as it is; an absent or empty one is no link and the page has no open-repo action.
pub fn repo_link(repo_url: Option<&str>) -> Option<String> {
    const PREFIX: &str = "https://raw.githubusercontent.com/";
    let url = repo_url.filter(|u| !u.is_empty())?;
    let converted = url.find(PREFIX).and_then(|at| {
        let rest = &url[at + PREFIX.len()..];
        let slashes: Vec<usize> = rest.match_indices('/').map(|(i, _)| i).collect();
        // the lazy groups take the first split that leaves a name, a slash, a name, a slash and one more character
        slashes.iter().enumerate().find_map(|(n, &first)| {
            let second = slashes[n + 1..].iter().find(|&&s| s > first + 1 && rest.len() > s + 1)?;
            (first > 0).then(|| format!("https://github.com/{}/{}", &rest[..first], &rest[first + 1..*second]))
        })
    });
    Some(converted.unwrap_or_else(|| url.to_owned()))
}

/// The text a click on the header copies (B§7.2). The block after the blank line is for installed extensions, which is
/// all this page shows. `appendLine` ends the Shared line with a newline, and the Repository line is added only when the
/// extension has a store (`AnimeExtensionDetailsScreen.kt:256-260`).
pub fn debug_text(d: &ExtDetails) -> String {
    let mut text = format!(
        "Extension name: {} (lang: {}; package: {})\nExtension version: {} (lib: {}; version code: {})\nNSFW: {}\n\nUpdate available: {}\nOrphaned: {}\nShared: {}\n",
        d.name,
        d.lang,
        d.pkg,
        d.version_name,
        d.lib_version.map_or_else(|| "n/a".to_owned(), |v| format!("{v:?}")),
        d.version_code,
        d.nsfw,
        d.has_update,
        d.obsolete,
        d.shared,
    );
    if let Some(url) = &d.repo_url {
        text.push_str("Repository: ");
        text.push_str(url);
    }
    text
}

/// The package name under the title: what follows the extension namespace, else the whole name.
pub fn stripped_pkg(pkg: &str) -> &str {
    ["eu.kanade.tachiyomi.extension.", "eu.kanade.tachiyomi.animeextension."]
        .iter()
        .find_map(|prefix| pkg.strip_prefix(prefix))
        .unwrap_or(pkg)
}

/// Clears the cookies of every distinct http base url through `remove`, which returns how many cookies it dropped. A
/// url with no base, or an empty one, is skipped. Returns the urls and the count; the page logs them and shows no
/// feedback (E-40-36, E-40-37).
pub fn clear_cookies(sources: &[DetailSource], remove: impl Fn(&str) -> usize) -> (Vec<String>, usize) {
    let bases: Vec<&str> = sources.iter().filter_map(|s| s.base_url.as_deref()).filter(|u| !u.is_empty()).collect();
    let urls = cookie_clear_targets(&bases);
    let cleared = urls.iter().map(|url| remove(url)).sum();
    (urls, cleared)
}

/// The log line of a cookie clear.
pub fn cleared_log(count: usize, urls: &[String]) -> String {
    format!("Cleared {count} cookies for: {}", urls.join(", "))
}

/// The banner after a cookie clear: the count the user sees (T-41a-021, a proposal over the silent log).
pub fn cleared_banner(count: usize) -> String {
    format!("Cleared {count} cookies")
}

#[cfg(test)]
mod banner_tests {
    use super::*;

    #[test]
    fn the_cookie_banner_shows_the_count_and_a_failed_url_counts_zero() {
        assert_eq!(cleared_banner(0), "Cleared 0 cookies");
        assert_eq!(cleared_banner(4), "Cleared 4 cookies");
        let (urls, cleared) = clear_cookies(&[DetailSource { id: 1, name: "A".into(), lang: "en".into(), label_as_name: false, configurable: false, base_url: Some("https://a.test/".into()) }], |_| 0);
        assert_eq!(cleared, 0, "a failed URL is counted as 0 (T25)");
        assert!(!urls.is_empty(), "the base URL is still asked");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_as_name_is_multi_source_and_not_one_name_repeated_per_language() {
        assert!(!label_as_name(&["Only"]), "one source is labelled by its language");
        assert!(!label_as_name(&["Same", "Same"]), "several languages of one source name are multi-lang single-source");
        assert!(label_as_name(&["Alpha", "Beta"]), "several different sources are labelled by name");
        assert!(label_as_name(&["Same", "Same", "Other"]), "any second name makes them distinct");
    }

    fn row(id: i64, name: &str, display: &str, label_as_name: bool, enabled: bool) -> SourceRow {
        SourceRow { id, name: name.into(), display_name: display.into(), label_as_name, enabled }
    }

    #[test]
    fn enabled_rows_come_first_and_each_group_sorts_by_its_label() {
        let mut rows = vec![
            row(1, "Zed", "Zed (EN)", true, false),
            row(2, "beta", "Beta (FR)", false, true),
            row(3, "Alpha", "ALPHA (EN)", false, true),
            row(4, "Mid", "mid", true, true),
        ];
        sort_rows(&mut rows);
        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        // The raw name keeps its case ("Mid" sorts before "alpha (en)"); only display names are lowercased (E-40-39).
        assert_eq!(ids, vec![4, 3, 2, 1], "enabled first; label is lowercased display name or raw name");
    }

    #[test]
    fn enable_all_and_disable_all_act_on_every_id_of_the_extension() {
        let disabled: BTreeSet<String> = ["9".to_owned()].into();
        let off = set_all_enabled(&disabled, &[1, 2], false);
        assert_eq!(off, ["1", "2", "9"].map(str::to_owned).into());
        let on = set_all_enabled(&off, &[1, 9], true);
        assert_eq!(on, ["2"].map(str::to_owned).into(), "enabling removes the ids, others stay disabled");
        assert_eq!(disabled.len(), 1, "the input set is not changed");
    }

    #[test]
    fn cookies_are_cleared_once_per_distinct_http_base_url() {
        let targets = cookie_clear_targets(&["https://a.test", "", "https://a.test", "ftp://x", "http://b.test"]);
        assert_eq!(targets, vec!["https://a.test".to_owned(), "http://b.test".to_owned()]);
        assert!(cookie_clear_targets(&[""]).is_empty());
    }

    fn details() -> ExtDetails {
        ExtDetails {
            name: "Foo".into(),
            pkg: "eu.kanade.tachiyomi.extension.en.foo".into(),
            lang: "en".into(),
            version_name: "1.4.12".into(),
            lib_version: Some(1.4),
            version_code: 12,
            nsfw: true,
            has_update: false,
            obsolete: true,
            shared: false,
            repo_url: Some("https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json".into()),
            icon_file: None,
            sources: Vec::new(),
        }
    }

    #[test]
    fn a_raw_github_url_becomes_the_repository_page_and_others_stay() {
        assert_eq!(repo_link(Some("https://raw.githubusercontent.com/user/repo/branch/index.min.json")).as_deref(), Some("https://github.com/user/repo"));
        assert_eq!(repo_link(Some("https://example.org/index.json")).as_deref(), Some("https://example.org/index.json"));
        assert_eq!(repo_link(Some("")), None, "an empty url is no link");
        assert_eq!(repo_link(None), None);
        // too short for the pattern: it stays as it is
        assert_eq!(repo_link(Some("https://raw.githubusercontent.com/user/repo")).as_deref(), Some("https://raw.githubusercontent.com/user/repo"));
        assert_eq!(repo_link(Some("https://raw.githubusercontent.com/user/repo/")).as_deref(), Some("https://raw.githubusercontent.com/user/repo/"), "one more character is needed");
        // found anywhere in the text, like Regex.find
        assert_eq!(repo_link(Some("x https://raw.githubusercontent.com/a/b/c")).as_deref(), Some("https://github.com/a/b"));
    }

    #[test]
    fn the_debug_text_is_the_block_of_the_header_click() {
        assert_eq!(
            debug_text(&details()),
            "Extension name: Foo (lang: en; package: eu.kanade.tachiyomi.extension.en.foo)\n\
             Extension version: 1.4.12 (lib: 1.4; version code: 12)\n\
             NSFW: true\n\n\
             Update available: false\n\
             Orphaned: true\n\
             Shared: false\n\
             Repository: https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json"
        );
        let mut none = details();
        none.repo_url = None;
        none.lib_version = Some(1.0);
        assert!(debug_text(&none).ends_with("Shared: false\n"), "no store: no Repository line, and the newline after Shared stays");
        assert!(!debug_text(&none).contains("Repository"));
        assert!(debug_text(&none).contains("lib: 1.0;"), "a whole lib version keeps its .0 like Kotlin's Double");
        none.lib_version = None;
        assert!(debug_text(&none).contains("lib: n/a;"), "an anime package has no library version");
    }

    #[test]
    fn the_package_under_the_title_loses_its_namespace() {
        assert_eq!(stripped_pkg("eu.kanade.tachiyomi.extension.en.foo"), "en.foo");
        assert_eq!(stripped_pkg("eu.kanade.tachiyomi.animeextension.ja.bar"), "ja.bar");
        assert_eq!(stripped_pkg("org.other.pkg"), "org.other.pkg");
    }

    fn source(id: i64, name: &str, lang: &str, label_as_name: bool, base: Option<&str>) -> DetailSource {
        DetailSource { id, name: name.into(), lang: lang.into(), label_as_name, configurable: id % 2 == 0, base_url: base.map(str::to_owned) }
    }

    #[test]
    fn rows_are_enabled_when_not_hidden_sorted_and_titled_by_name_or_language() {
        let sources = vec![source(1, "Zed", "en", true, None), source(2, "Beta", "fr", false, None), source(3, "Alpha", "en", true, None)];
        let rows = detail_rows(&sources, &["1".to_owned()].into());
        let ids: Vec<i64> = rows.iter().map(|r| r.row.id).collect();
        assert_eq!(ids, [3, 2, 1], "enabled first, by label; the hidden one last");
        assert!(!rows[2].row.enabled);
        assert_eq!(rows[0].title(), "Alpha", "a label-as-name source shows its name");
        assert_eq!(rows[1].title(), crate::browse::names::localized_display_name("fr"), "else the language name");
        assert!(rows[1].configurable && !rows[0].configurable, "only a configurable source has the settings button");
    }

    #[test]
    fn cookies_are_cleared_per_distinct_base_url_and_the_count_is_summed() {
        let sources = vec![
            source(1, "A", "en", true, Some("https://a.test")),
            source(2, "B", "en", true, Some("https://a.test")),
            source(3, "C", "en", true, Some("")),
            source(4, "D", "en", true, None),
            source(5, "E", "en", true, Some("http://b.test")),
        ];
        let seen = std::cell::RefCell::new(Vec::new());
        let (urls, cleared) = clear_cookies(&sources, |url| {
            seen.borrow_mut().push(url.to_owned());
            if url.starts_with("https") { 3 } else { 2 }
        });
        assert_eq!(urls, ["https://a.test", "http://b.test"]);
        assert_eq!(*seen.borrow(), urls, "each url once");
        assert_eq!(cleared, 5);
        assert_eq!(cleared_log(cleared, &urls), "Cleared 5 cookies for: https://a.test, http://b.test");
    }
}
