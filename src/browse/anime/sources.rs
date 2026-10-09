//! The anime sources list and the Sources tab state (T-41b-001 to T-41b-005, T-41b-007, T-41b-008, T-41b-041,
//! T-41a-017, T-41a-042, T-41a-043). Ports `GetEnabledAnimeSources`, `AnimeSourcesScreenModel` and the visibility page
//! model. The row filter and the rows come from the parent module; the section order is the anime one (decision D-1 of
//! `p2-topic-sources-list.md`): last used, pinned, then languages with `all` first, the labels in English, and the empty
//! language last.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use kuro_domain::source::browse::BrowseQuery;
use kuro_domain::source::languages::{display_tag, utf16_cmp};
use kuro_domain::source::model::Source;
use kuro_i18n::names::english_name;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::source::{SourcePrefs, system_locale};

use crate::browse::names::{case_insensitive_cmp, source_display_name};
use crate::browse::primitives::{ToastDuration, ToastSpec};
use crate::browse::sources::{HEADER_ACTIONS, ListInput, LOCAL_SOURCE_ID, Section, SourceRow, SourceToggles, SourcesTabAction, source_list};

/// The stored source sets the anime sources list reads. Ids are decimal strings, as stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListPrefs {
    pub enabled_langs: BTreeSet<String>,
    pub hidden: BTreeSet<String>,
    pub pinned: BTreeSet<String>,
    pub last_used: i64,
    /// The system locale, which names the empty language (E-SRC-04).
    pub locale: String,
}

impl ListPrefs {
    /// Reads `source_languages`, `hidden_anime_catalogues`, `pinned_anime_catalogues` and the last-used anime source.
    pub fn read(prefs: &SourcePrefs) -> Self {
        Self {
            enabled_langs: prefs.enabled_languages.get(),
            hidden: prefs.disabled_anime_sources.get(),
            pinned: prefs.pinned_anime_sources.get(),
            last_used: prefs.last_used_anime_source.get(),
            locale: system_locale(),
        }
    }
}

/// The rows of the anime sources list. A source stays when its language is enabled or it is the local source, and a
/// hidden source is dropped. Rows sort by name ignoring case. A pinned source sits under Pinned, and the last-used
/// source also appears once under Last used (E-SRC-03).
pub fn enabled_source_rows(sources: &[Source], prefs: &ListPrefs) -> Vec<SourceRow> {
    let rows = source_list(&ListInput {
        sources,
        enabled_languages: &prefs.enabled_langs,
        disabled: &prefs.hidden,
        pinned: &prefs.pinned,
        last_used: prefs.last_used,
    });
    regroup(rows, &prefs.locale)
}

/// The integers of a stored id set. An entry that is not an integer is skipped with a log line and never rewritten
/// (E-SRC-07).
pub fn parse_ids(raw: &BTreeSet<String>) -> BTreeSet<i64> {
    raw.iter()
        .filter_map(|entry| match entry.parse::<i64>() {
            Ok(id) => Some(id),
            Err(_) => {
                tracing::warn!(entry = %entry, "skipping a source id that is not an integer");
                None
            }
        })
        .collect()
}

/// The header text of a language tag (T-41a-017, T-41b-041). `all` reads Multi, `other` reads Other, the empty tag
/// reads the system language (`system_language_label`), and the rest read their English display names. `zh-CN` and
/// `zh-TW` are looked up as Hans and Hant, where the name table keys them by their own tags.
pub fn language_label(tag: &str, locale: &str) -> String {
    match tag {
        "" => system_language_label(locale),
        "all" | "other" => source_display_name(tag),
        other => {
            let named = english_name(other);
            if named == other { english_name(display_tag(other)) } else { named }
        }
    }
}

/// The English name of the system language, which labels the empty language (E-SRC-04). `Other` when the locale names no
/// language the name table knows.
pub fn system_language_label(locale: &str) -> String {
    let tag = locale.split(['.', '@', '_', '-']).next().unwrap_or_default().to_ascii_lowercase();
    let named = english_name(&tag);
    if tag.is_empty() || named == tag { source_display_name("other") } else { named }
}

/// The header text of a section: the two special names, or the language label.
pub fn anime_section_title(section: &Section, locale: &str) -> String {
    match section {
        Section::LastUsed => source_display_name("last_used"),
        Section::Pinned => source_display_name("pinned"),
        Section::Language(tag) => language_label(tag, locale),
    }
}

/// Section order: last used, pinned, then the languages (`language_cmp`).
pub fn anime_section_cmp(a: &Section, b: &Section, locale: &str) -> Ordering {
    fn rank(section: &Section) -> u8 {
        match section {
            Section::LastUsed => 0,
            Section::Pinned => 1,
            Section::Language(_) => 2,
        }
    }
    match (a, b) {
        (Section::Language(x), Section::Language(y)) => language_cmp(x, y, locale),
        _ => rank(a).cmp(&rank(b)),
    }
}

/// Decision D-1: the empty language is last, `all` is first, and the others order by their English labels, with the tag
/// breaking a tie between two tags that share a label.
pub fn language_cmp(x: &str, y: &str, locale: &str) -> Ordering {
    match (x.is_empty(), y.is_empty()) {
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        (true, true) => return Ordering::Equal,
        _ => {}
    }
    match (x == "all", y == "all") {
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        _ => {}
    }
    utf16_cmp(&language_label(x, locale), &language_label(y, locale)).then_with(|| utf16_cmp(x, y))
}

/// Puts the section groups of `rows` in the anime order, each group keeping its rows in the order it had.
fn regroup(rows: Vec<SourceRow>, locale: &str) -> Vec<SourceRow> {
    let mut groups: Vec<(Section, Vec<SourceRow>)> = Vec::new();
    for row in rows {
        match row {
            SourceRow::Header(section) => groups.push((section, Vec::new())),
            item => {
                if let Some((_, items)) = groups.last_mut() {
                    items.push(item);
                }
            }
        }
    }
    groups.sort_by(|(a, _), (b, _)| anime_section_cmp(a, b, locale));
    groups.into_iter().flat_map(|(section, items)| std::iter::once(SourceRow::Header(section)).chain(items)).collect()
}

/// One language section of the visibility page. Each source is paired with its visible flag (not hidden).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityGroup {
    pub lang: String,
    pub enabled: bool,
    pub sources: Vec<(Source, bool)>,
}

/// The visibility page's sections. Only installed sources count: stubs and the local source are left out (E-VIS-04).
/// Enabled languages come first, then `all`, then the other languages by label (`language_cmp`). Inside a section,
/// visible sources come first, then the rest, each by name.
pub fn visibility_groups(sources: &[Source], enabled_langs: &BTreeSet<String>, hidden: &BTreeSet<i64>, locale: &str) -> Vec<VisibilityGroup> {
    let mut groups: Vec<VisibilityGroup> = Vec::new();
    for source in sources.iter().filter(|s| !s.is_stub && s.id != LOCAL_SOURCE_ID) {
        let visible = !hidden.contains(&source.id);
        match groups.iter_mut().find(|g| g.lang == source.lang) {
            Some(group) => group.sources.push((source.clone(), visible)),
            None => groups.push(VisibilityGroup {
                lang: source.lang.clone(),
                enabled: enabled_langs.contains(&source.lang),
                sources: vec![(source.clone(), visible)],
            }),
        }
    }
    for group in &mut groups {
        group.sources.sort_by(|(a, va), (b, vb)| (!*va).cmp(&!*vb).then_with(|| case_insensitive_cmp(&a.name, &b.name)));
    }
    groups.sort_by(|a, b| (!a.enabled).cmp(&!b.enabled).then_with(|| language_cmp(&a.lang, &b.lang, locale)));
    groups
}

/// The Sources tab state. Every emission of the watch replaces the rows. A failure before the first emission is an
/// inline error with Retry (E-SRC-02). A failure after it keeps the last rows and returns a toast.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcesTab {
    pub loading: bool,
    pub rows: Vec<SourceRow>,
    /// The options dialog of one source, while it is open. A header never opens one, so the payload is the source.
    pub dialog: Option<Source>,
    /// The inline error row with Retry. Only shown before the first emission.
    pub inline_error: bool,
    emitted: bool,
}

impl SourcesTab {
    pub fn new() -> Self {
        Self { loading: true, rows: Vec::new(), dialog: None, inline_error: false, emitted: false }
    }

    /// No source row at all: the page says "No source found" (`source_empty_screen`).
    pub fn is_empty(&self) -> bool {
        !self.rows.iter().any(|row| matches!(row, SourceRow::Item(_)))
    }

    pub fn emit(&mut self, rows: Vec<SourceRow>) {
        self.rows = rows;
        self.loading = false;
        self.inline_error = false;
        self.emitted = true;
    }

    /// The watch failed. Returns the toast to show, or `None` when the failure is shown inline instead.
    pub fn fail(&mut self) -> Option<ToastSpec> {
        self.loading = false;
        if self.emitted {
            Some(list_failure_toast())
        } else {
            self.inline_error = true;
            None
        }
    }

    /// Retry subscribes to the watch again and clears the error row.
    pub fn retry(&mut self) {
        self.inline_error = false;
        self.loading = true;
    }
}

impl Default for SourcesTab {
    fn default() -> Self {
        Self::new()
    }
}

/// The toast of a failed watch after the first list: the shared `internal_error` text for 4 s (T-41b-004).
pub fn list_failure_toast() -> ToastSpec {
    ToastSpec { message: tr(MsgKey::internal_error), action: None, duration: ToastDuration::Seconds(4) }
}

/// Where a Sources tab action or row leads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabRoute {
    /// Global search with an empty query.
    GlobalSearch,
    /// The visibility page.
    Visibility,
    /// One source's browse page on the given listing.
    Browse { source_id: i64, query: BrowseQuery },
}

/// The header actions: Global search, then Filter (which opens the visibility page).
pub fn header_route(action: SourcesTabAction) -> TabRoute {
    match action {
        SourcesTabAction::GlobalSearch => TabRoute::GlobalSearch,
        SourcesTabAction::Filter => TabRoute::Visibility,
    }
}

/// A row opens the source's Popular listing. Its Latest button opens the Latest listing.
pub fn row_route(source_id: i64, latest: bool) -> TabRoute {
    let query = if latest { BrowseQuery::Latest } else { BrowseQuery::Popular };
    TabRoute::Browse { source_id, query }
}

/// The header actions in display order.
pub fn header_actions() -> [SourcesTabAction; 2] {
    HEADER_ACTIONS
}

/// The pin and disable writers over `pinned_anime_catalogues` and `hidden_anime_catalogues`.
pub fn anime_toggles(prefs: &SourcePrefs) -> SourceToggles {
    SourceToggles::new(prefs.pinned_anime_sources.clone(), prefs.disabled_anime_sources.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::anime::fixtures::worked_example;
    use kuro_domain::source::pin::{Pin, Pins};
    use kuro_prefs::source::default_enabled_languages;
    use kuro_prefs::store::PrefStore;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn prefs(langs: &[&str], hidden: &[&str], pinned: &[&str], last_used: i64) -> ListPrefs {
        ListPrefs { enabled_langs: set(langs), hidden: set(hidden), pinned: set(pinned), last_used, locale: "en_US.UTF-8".into() }
    }

    fn item_ids(rows: &[SourceRow]) -> Vec<(i64, bool)> {
        rows.iter()
            .filter_map(|row| match row {
                SourceRow::Item(source) => Some((source.id, source.is_used_last)),
                SourceRow::Header(_) => None,
            })
            .collect()
    }

    fn headers(rows: &[SourceRow]) -> Vec<Section> {
        rows.iter()
            .filter_map(|row| match row {
                SourceRow::Header(section) => Some(section.clone()),
                SourceRow::Item(_) => None,
            })
            .collect()
    }

    fn lang(tag: &str) -> Section {
        Section::Language(tag.into())
    }

    #[test]
    fn a_language_that_is_not_enabled_is_dropped_and_the_local_source_is_kept() {
        let sources = [Source::new(1, "Alpha", "en"), Source::new(2, "Beta", "fr"), Source::new(0, "Local", "")];
        let rows = enabled_source_rows(&sources, &prefs(&["en"], &[], &[], -1));
        assert_eq!(item_ids(&rows), [(1, false), (0, false)]);
    }

    #[test]
    fn a_hidden_source_is_dropped_from_the_list() {
        let sources = [Source::new(1, "Alpha", "en"), Source::new(2, "Beta", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en"], &["2"], &[], -1));
        assert_eq!(item_ids(&rows), [(1, false)]);
    }

    #[test]
    fn rows_sort_by_name_ignoring_case_only() {
        let sources = [Source::new(6, "Banana", "en"), Source::new(5, "apple", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en"], &[], &[], -1));
        assert_eq!(item_ids(&rows), [(5, false), (6, false)]);
    }

    #[test]
    fn two_sources_with_one_name_and_language_keep_their_input_order() {
        let sources = [Source::new(3, "Same", "en"), Source::new(4, "Same", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en"], &[], &[], -1));
        assert_eq!(item_ids(&rows), [(3, false), (4, false)]);
    }

    #[test]
    fn the_last_used_source_adds_one_copy_under_last_used_before_its_language_group() {
        let sources = [Source::new(1, "Alpha", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en"], &[], &[], 1));
        assert_eq!(headers(&rows), [Section::LastUsed, lang("en")]);
        assert_eq!(item_ids(&rows), [(1, true), (1, false)]);
    }

    #[test]
    fn a_pinned_last_used_source_shows_under_both_last_used_and_pinned() {
        let sources = [Source::new(1, "Alpha", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en"], &[], &["1"], 1));
        assert_eq!(headers(&rows), [Section::LastUsed, Section::Pinned]);
        assert_eq!(item_ids(&rows), [(1, true), (1, false)]);
    }

    #[test]
    fn a_pinned_id_for_a_source_that_is_not_installed_makes_no_row() {
        let sources = [Source::new(1, "Alpha", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en"], &[], &["99"], -1));
        assert_eq!(headers(&rows), [lang("en")]);
        assert_eq!(item_ids(&rows), [(1, false)]);
    }

    #[test]
    fn pinned_sources_come_before_the_language_groups() {
        let sources = [Source::new(1, "x", "fr"), Source::new(2, "y", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en", "fr"], &[], &["1"], -1));
        assert_eq!(headers(&rows), [Section::Pinned, lang("en")], "the pinned fr source leaves no fr group");
    }

    #[test]
    fn language_sections_put_all_first_and_the_empty_language_last() {
        let sources = [Source::new(1, "a", ""), Source::new(2, "b", "fr"), Source::new(3, "c", "all")];
        let rows = enabled_source_rows(&sources, &prefs(&["all", "fr", ""], &[], &[], -1));
        assert_eq!(headers(&rows), [lang("all"), lang("fr"), lang("")]);
    }

    #[test]
    fn languages_order_by_english_label_not_by_tag() {
        let sources = [Source::new(1, "Japan", "ja"), Source::new(2, "China", "zh-CN")];
        let rows = enabled_source_rows(&sources, &prefs(&["ja", "zh-CN"], &[], &[], -1));
        assert_eq!(headers(&rows), [lang("zh-CN"), lang("ja")], "Chinese (Simplified) sorts before Japanese");
    }

    #[test]
    fn the_worked_example_gives_the_listed_sections_and_members() {
        let rows = enabled_source_rows(&worked_example(), &prefs(&["all", "en", "ar"], &[], &["12"], 12));
        assert_eq!(headers(&rows), [Section::LastUsed, Section::Pinned, lang("all"), lang("ar"), lang("en"), lang("other")]);
        assert_eq!(
            item_ids(&rows),
            [(12, true), (12, false), (13, false), (14, false), (11, false), (16, false), (0, false)],
            "Epsilon with the empty language is dropped; Alpha before Zeta"
        );
    }

    #[test]
    fn no_enabled_language_and_no_local_source_leaves_the_list_empty() {
        let sources = [Source::new(1, "Alpha", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["fr"], &[], &[], -1));
        let mut tab = SourcesTab::new();
        tab.emit(rows);
        assert!(tab.is_empty(), "the page says No source found");
    }

    #[test]
    fn a_disabled_local_source_id_hides_the_local_source_from_the_list() {
        let sources = [Source::new(0, "Local", "other"), Source::new(1, "Alpha", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en"], &["0"], &[], -1));
        assert_eq!(item_ids(&rows), [(1, false)], "the hidden set applies to id 0 too; only the menu hides Disable");
    }

    #[test]
    fn the_last_used_duplicate_follows_the_pin_toggle() {
        let store = PrefStore::in_memory();
        let source_prefs = SourcePrefs::new(store);
        source_prefs.enabled_languages.set(set(&["en"])).expect("store");
        source_prefs.last_used_anime_source.set(1).expect("store");
        let sources = [Source::new(1, "Alpha", "en")];
        let toggles = anime_toggles(&source_prefs);
        assert!(toggles.toggle_pin(1).expect("pin on"));
        let pinned = enabled_source_rows(&sources, &ListPrefs::read(&source_prefs));
        assert_eq!(headers(&pinned), [Section::LastUsed, Section::Pinned]);
        assert!(!toggles.toggle_pin(1).expect("pin off"));
        let unpinned = enabled_source_rows(&sources, &ListPrefs::read(&source_prefs));
        assert_eq!(headers(&unpinned), [Section::LastUsed, lang("en")]);
    }

    #[test]
    fn a_bulk_disable_and_enable_write_every_id_in_one_set() {
        let store = PrefStore::in_memory();
        let source_prefs = SourcePrefs::new(store);
        let toggles = anime_toggles(&source_prefs);
        assert!(toggles.set_disabled_many(&[1, 2], true).expect("hide"));
        assert_eq!(source_prefs.disabled_anime_sources.get(), set(&["1", "2"]));
        assert!(!toggles.set_disabled_many(&[1, 2], false).expect("show"));
        assert!(source_prefs.disabled_anime_sources.get().is_empty());
    }

    #[test]
    fn the_empty_language_reads_the_system_language_or_other() {
        assert_eq!(language_label("", "de_DE.UTF-8"), "German");
        assert_eq!(language_label("", "C"), "Other");
        assert_eq!(language_label("", ""), "Other");
    }

    #[test]
    fn the_special_languages_have_their_labels_and_zh_tags_read_as_their_names() {
        assert_eq!(language_label("all", "C"), "Multi");
        assert_eq!(language_label("other", "C"), "Other");
        assert_eq!(language_label("zh-CN", "C"), "Chinese (Simplified)");
        assert_eq!(language_label("zh-TW", "C"), "Chinese (Traditional)");
        assert_eq!(language_label("ar", "C"), "Arabic");
    }

    #[test]
    fn section_titles_are_the_special_names_or_the_language_label() {
        assert_eq!(anime_section_title(&Section::LastUsed, "C"), "Last used");
        assert_eq!(anime_section_title(&Section::Pinned, "C"), "Pinned");
        assert_eq!(anime_section_title(&lang("all"), "C"), "Multi");
        assert_eq!(anime_section_title(&lang(""), "es_ES.UTF-8"), "Spanish");
    }

    #[test]
    fn section_order_is_all_first_then_labels_then_empty_last() {
        assert_eq!(language_cmp("all", "en", "C"), Ordering::Less);
        assert_eq!(language_cmp("ar", "", "C"), Ordering::Less, "the empty language is last");
        assert_eq!(language_cmp("", "all", "C"), Ordering::Greater);
        assert_eq!(anime_section_cmp(&Section::Pinned, &lang("all"), "C"), Ordering::Less);
    }

    #[test]
    fn visibility_sections_list_enabled_then_all_then_names_and_skip_stubs_and_local() {
        let mut stub = Source::new(5, "Stub", "en");
        stub.is_stub = true;
        let sources = [
            Source::new(1, "Zed", "en"),
            Source::new(2, "Alpha", "en"),
            Source::new(3, "Mid", "all"),
            Source::new(4, "Fra", "fr"),
            stub,
            Source::new(0, "Local", ""),
        ];
        let enabled = set(&["all", "en"]);
        let hidden = BTreeSet::from([2_i64]);
        let groups = visibility_groups(&sources, &enabled, &hidden, "C");
        let langs: Vec<&str> = groups.iter().map(|g| g.lang.as_str()).collect();
        assert_eq!(langs, ["all", "en", "fr"]);
        let en = &groups[1];
        let names: Vec<(&str, bool)> = en.sources.iter().map(|(s, visible)| (s.name.as_str(), *visible)).collect();
        assert_eq!(names, [("Zed", true), ("Alpha", false)], "visible sources first");
        assert!(!groups[2].enabled, "fr is not enabled");
    }

    #[test]
    fn an_empty_source_set_gives_no_visibility_groups() {
        assert!(visibility_groups(&[], &set(&["en"]), &BTreeSet::new(), "C").is_empty());
    }

    #[test]
    fn the_tab_starts_loading_and_an_emission_clears_loading_and_sets_empty_when_no_item() {
        let mut tab = SourcesTab::new();
        assert!(tab.loading && tab.is_empty());
        assert!(tab.dialog.is_none());
        tab.emit(vec![SourceRow::Header(Section::Pinned)]);
        assert!(!tab.loading);
        assert!(tab.is_empty(), "a header alone is not a source");
        tab.emit(vec![SourceRow::Item(Source::new(1, "a", "en"))]);
        assert!(!tab.is_empty());
    }

    #[test]
    fn a_failure_before_the_first_emission_is_an_inline_retry_and_retry_clears_it() {
        let mut tab = SourcesTab::new();
        assert!(tab.fail().is_none());
        assert!(tab.inline_error && !tab.loading);
        tab.retry();
        assert!(!tab.inline_error && tab.loading);
    }

    #[test]
    fn a_failure_after_the_first_emission_keeps_the_rows_and_shows_the_four_second_toast() {
        let mut tab = SourcesTab::new();
        let rows = vec![SourceRow::Item(Source::new(1, "a", "en"))];
        tab.emit(rows.clone());
        let toast = tab.fail().expect("toast after the first list");
        assert_eq!(toast.message, "InternalError: Check crash logs for further information");
        assert_eq!(toast.duration, ToastDuration::Seconds(4));
        assert_eq!(tab.rows, rows, "the last list stays");
        assert!(!tab.inline_error);
    }

    #[test]
    fn the_empty_state_and_header_texts_come_from_the_string_table() {
        assert_eq!(tr(MsgKey::source_empty_screen), "No source found");
        assert_eq!(tr(MsgKey::last_used_source), "Last used");
        assert_eq!(tr(MsgKey::pinned_sources), "Pinned");
    }

    #[test]
    fn header_actions_and_rows_route_to_the_right_pages() {
        assert_eq!(header_actions(), HEADER_ACTIONS);
        assert_eq!(header_route(SourcesTabAction::GlobalSearch), TabRoute::GlobalSearch);
        assert_eq!(header_route(SourcesTabAction::Filter), TabRoute::Visibility);
        assert_eq!(row_route(7, false), TabRoute::Browse { source_id: 7, query: BrowseQuery::Popular });
        assert_eq!(row_route(7, true), TabRoute::Browse { source_id: 7, query: BrowseQuery::Latest });
    }

    #[test]
    fn the_default_enabled_languages_are_all_english_and_the_system_language() {
        assert_eq!(default_enabled_languages("de_DE.UTF-8"), set(&["all", "de", "en"]));
        assert_eq!(default_enabled_languages("C"), set(&["all", "en"]));
    }

    #[test]
    fn pin_bits_read_the_pinned_bit_and_ids_that_are_not_integers_are_skipped() {
        assert_eq!((Pin::Pinned as u8, Pin::Actual as u8), (0b01, 0b10));
        assert_eq!(Pins::PINNED.bits(), 0b11, "PINNED is Pinned | Actual");
        assert!(Pins::PINNED.is_pinned());
        assert!(!Pins::UNPINNED.is_pinned());
        assert!(!Pins::UNPINNED.with(Pin::Actual).is_pinned(), "the last-used copy is not pinned");
        assert_eq!(parse_ids(&set(&["3", "x", "4.0", "-2"])), BTreeSet::from([-2_i64, 3]));
    }

    #[test]
    fn a_toggle_writes_the_decimal_id_and_keeps_entries_that_are_not_ids() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        prefs.pinned_anime_sources.set(set(&["abc"])).expect("store");
        let toggles = anime_toggles(&prefs);
        assert!(toggles.toggle_pin(5).expect("pin on"));
        assert_eq!(prefs.pinned_anime_sources.get(), set(&["abc", "5"]));
        assert!(!toggles.toggle_pin(5).expect("pin off"));
        assert_eq!(prefs.pinned_anime_sources.get(), set(&["abc"]), "the unparseable entry is never rewritten");
    }

    #[test]
    fn list_prefs_read_the_anime_keys_and_the_last_used_source() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        prefs.disabled_anime_sources.set(set(&["9"])).expect("store");
        prefs.last_used_anime_source.set(4).expect("store");
        let read = ListPrefs::read(&prefs);
        assert_eq!((read.hidden, read.last_used), (set(&["9"]), 4));
        assert!(read.enabled_langs.contains("en"), "the default languages come from the locale");
    }
}
