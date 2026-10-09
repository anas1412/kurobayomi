//! The anime sources list and the Sources tab state (T-41b-001 to T-41b-005, T-41b-007, T-41b-008, T-41b-041,
//! T-41a-042, T-41a-043). Ports `GetEnabledAnimeSources`, `AnimeSourcesScreenModel` and the visibility page model. The
//! grouping, the pin and disable writers and the header actions are the parent module's, fed with the anime keys.

use std::collections::BTreeSet;

use kuro_domain::source::browse::BrowseQuery;
use kuro_domain::source::model::Source;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::source::SourcePrefs;

use crate::browse::names::{case_insensitive_cmp, compare_languages};
use crate::browse::primitives::{ToastDuration, ToastSpec};
use crate::browse::sources::{HEADER_ACTIONS, ListInput, LOCAL_SOURCE_ID, SourceRow, SourceToggles, SourcesTabAction, source_list};

/// The stored source sets the anime sources list reads. Ids are decimal strings, as stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListPrefs {
    pub enabled_langs: BTreeSet<String>,
    pub hidden: BTreeSet<String>,
    pub pinned: BTreeSet<String>,
    pub last_used: i64,
}

impl ListPrefs {
    /// Reads `source_languages`, `hidden_anime_catalogues`, `pinned_anime_catalogues` and the last-used anime source.
    pub fn read(prefs: &SourcePrefs) -> Self {
        Self {
            enabled_langs: prefs.enabled_languages.get(),
            hidden: prefs.disabled_anime_sources.get(),
            pinned: prefs.pinned_anime_sources.get(),
            last_used: prefs.last_used_anime_source.get(),
        }
    }
}

/// The rows of the anime sources list. A source stays when its language is enabled or it is the local source, and a
/// hidden source is dropped. Rows sort by name ignoring case. A pinned source sits under Pinned, and the last-used
/// source also appears once under Last used (E-SRC-03).
pub fn enabled_source_rows(sources: &[Source], prefs: &ListPrefs) -> Vec<SourceRow> {
    source_list(&ListInput {
        sources,
        enabled_languages: &prefs.enabled_langs,
        disabled: &prefs.hidden,
        pinned: &prefs.pinned,
        last_used: prefs.last_used,
    })
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

/// One language section of the visibility page. Each source is paired with its visible flag (not hidden).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityGroup {
    pub lang: String,
    pub enabled: bool,
    pub sources: Vec<(Source, bool)>,
}

/// The visibility page's sections. Only installed sources count: stubs and the local source are left out (E-VIS-04).
/// Enabled languages come first, then `all`, then the other languages by display name. Inside a section, visible
/// sources come first, then the rest, each by name.
pub fn visibility_groups(sources: &[Source], enabled_langs: &BTreeSet<String>, hidden: &BTreeSet<i64>) -> Vec<VisibilityGroup> {
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
    groups.sort_by(|a, b| (!a.enabled).cmp(&!b.enabled).then_with(|| compare_languages(&a.lang, &b.lang)));
    groups
}

/// The Sources tab state. Every emission of the watch replaces the rows. A failure before the first emission is an
/// inline error with Retry (E-SRC-02). A failure after it keeps the last rows and returns a toast.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcesTab {
    pub loading: bool,
    pub rows: Vec<SourceRow>,
    /// The options dialog of one source, while it is open.
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
    use crate::browse::sources::{HEADER_ACTIONS, Section};
    use kuro_domain::source::pin::{Pin, Pins};
    use kuro_prefs::source::default_enabled_languages;
    use kuro_prefs::store::PrefStore;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn prefs(langs: &[&str], hidden: &[&str], pinned: &[&str], last_used: i64) -> ListPrefs {
        ListPrefs { enabled_langs: set(langs), hidden: set(hidden), pinned: set(pinned), last_used }
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
    fn the_last_used_source_adds_one_copy_under_last_used_before_its_language_group() {
        let sources = [Source::new(1, "Alpha", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en"], &[], &[], 1));
        assert_eq!(headers(&rows), [Section::LastUsed, Section::Language("en".into())]);
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
    fn pinned_sources_come_before_the_language_groups() {
        let sources = [Source::new(1, "x", "fr"), Source::new(2, "y", "en")];
        let rows = enabled_source_rows(&sources, &prefs(&["en", "fr"], &[], &["1"], -1));
        assert_eq!(headers(&rows), [Section::Pinned, Section::Language("en".into())], "the pinned fr source leaves no fr group");
    }

    #[test]
    fn language_sections_put_all_first_and_the_unknown_language_last() {
        let sources = [Source::new(1, "a", ""), Source::new(2, "b", "fr"), Source::new(3, "c", "all")];
        let rows = enabled_source_rows(&sources, &prefs(&["all", "fr", ""], &[], &[], -1));
        assert_eq!(headers(&rows), [Section::Language("all".into()), Section::Language("fr".into()), Section::Language(String::new())]);
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
        let groups = visibility_groups(&sources, &enabled, &hidden);
        let langs: Vec<&str> = groups.iter().map(|g| g.lang.as_str()).collect();
        assert_eq!(langs, ["all", "en", "fr"]);
        let en = &groups[1];
        let names: Vec<(&str, bool)> = en.sources.iter().map(|(s, visible)| (s.name.as_str(), *visible)).collect();
        assert_eq!(names, [("Zed", true), ("Alpha", false)], "visible sources first");
        assert!(!groups[2].enabled, "fr is not enabled");
    }

    #[test]
    fn an_empty_source_set_gives_no_visibility_groups() {
        assert!(visibility_groups(&[], &set(&["en"]), &BTreeSet::new()).is_empty());
    }

    #[test]
    fn the_tab_starts_loading_and_an_emission_clears_loading_and_sets_empty_when_no_item() {
        let mut tab = SourcesTab::new();
        assert!(tab.loading && tab.is_empty());
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
    fn section_titles_and_language_order_follow_the_locale_rules() {
        assert_eq!(crate::browse::sources::section_title(&Section::LastUsed), "Last used");
        assert_eq!(crate::browse::sources::section_title(&Section::Pinned), "Pinned");
        assert_eq!(crate::browse::sources::section_title(&Section::Language("all".into())), "Multi");
        assert_eq!(crate::browse::sources::section_title(&Section::Language(String::new())), "Other");
        let hans = crate::browse::sources::section_title(&Section::Language("zh-CN".into()));
        assert_eq!(hans, crate::browse::sources::section_title(&Section::Language("zh-Hans".into())));
        assert_ne!(hans, "zh-CN");
        assert_eq!(compare_languages("all", "en"), std::cmp::Ordering::Less);
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
