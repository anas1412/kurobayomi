//! The installed-sources list: grouping and order, pin and disable toggles, the options dialog and the Sources tab
//! actions (T-40a-007, T-40a-008, T-40b-010, T-40b-011, T-40b-012, T-40b-014). Ports
//! `GetEnabledMangaSources` and `MangaSourcesScreenModel.collectLatestSources`.

use std::cmp::Ordering;
use std::collections::BTreeSet;
#[cfg(test)]
use std::sync::Mutex;

use kuro_domain::source::model::Source;
use kuro_domain::source::pin::{Pin, Pins};
#[cfg(test)]
use kuro_domain::source::toggles::{flip_disabled, toggle_pin};
#[cfg(test)]
use kuro_prefs::PrefError;
#[cfg(test)]
use kuro_prefs::pref::Pref;

use super::names::{case_insensitive_cmp, localized_display_name, source_display_name};
#[cfg(test)]
use super::primitives::{ToastDuration, ToastSpec};

/// The local source has id 0. It is never dropped by the language filter (`LocalMangaSource.ID`).
pub const LOCAL_SOURCE_ID: i64 = 0;

/// A section header of the sources list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Section {
    LastUsed,
    Pinned,
    /// A language code; `""` (unknown language) sorts last.
    Language(String),
}

/// One row of the sources list: a section header or a source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceRow {
    Header(Section),
    Item(Source),
}

/// The inputs of one list build, read from the preferences on the GTK thread.
pub struct ListInput<'a> {
    pub sources: &'a [Source],
    /// `source_languages`: language codes compared exactly.
    pub enabled_languages: &'a BTreeSet<String>,
    /// `hidden_catalogues`: decimal source ids.
    pub disabled: &'a BTreeSet<String>,
    /// `pinned_catalogues`: decimal source ids.
    pub pinned: &'a BTreeSet<String>,
    /// `last_catalogue_source`, -1 for none.
    pub last_used: i64,
}

/// Section order: last used, pinned, then language codes in UTF-16 order with `""` last.
pub fn section_cmp(a: &Section, b: &Section) -> Ordering {
    fn rank(section: &Section) -> u8 {
        match section {
            Section::LastUsed => 0,
            Section::Pinned => 1,
            Section::Language(_) => 2,
        }
    }
    match (a, b) {
        (Section::Language(x), Section::Language(y)) => match (x.is_empty(), y.is_empty()) {
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => x.encode_utf16().cmp(y.encode_utf16()),
        },
        _ => rank(a).cmp(&rank(b)),
    }
}

/// Builds the sectioned list. A pinned source is listed under Pinned. A last-used source also appears once more, as its
/// own copy under Last used (E-SL-04). Languages that are not enabled and disabled sources are dropped (E-SL-01, E-SL-02).
pub fn source_list(input: &ListInput) -> Vec<SourceRow> {
    let mut visible: Vec<&Source> = input
        .sources
        .iter()
        .filter(|s| input.enabled_languages.contains(&s.lang) || s.id == LOCAL_SOURCE_ID)
        .filter(|s| !input.disabled.contains(&s.id.to_string()))
        .collect();
    visible.sort_by(|a, b| case_insensitive_cmp(&a.name, &b.name));

    let mut flat: Vec<Source> = Vec::new();
    for source in visible {
        let mut copy = source.clone();
        copy.pin = if input.pinned.contains(&source.id.to_string()) { Pins::PINNED } else { Pins::UNPINNED };
        copy.is_used_last = false;
        flat.push(copy.clone());
        if source.id == input.last_used {
            copy.is_used_last = true;
            copy.pin = copy.pin.minus(Pin::Actual);
            flat.push(copy);
        }
    }

    let mut groups: Vec<(Section, Vec<Source>)> = Vec::new();
    for source in flat {
        let section = if source.is_used_last {
            Section::LastUsed
        } else if source.pin.contains(Pin::Actual) {
            Section::Pinned
        } else {
            Section::Language(source.lang.clone())
        };
        match groups.iter_mut().find(|(s, _)| *s == section) {
            Some((_, items)) => items.push(source),
            None => groups.push((section, vec![source])),
        }
    }
    groups.sort_by(|(a, _), (b, _)| section_cmp(a, b));

    groups
        .into_iter()
        .flat_map(|(section, items)| {
            std::iter::once(SourceRow::Header(section)).chain(items.into_iter().map(SourceRow::Item))
        })
        .collect()
}

/// The header label of a section: the four special names, or the language's own name (T-40a-033). A source with no
/// language sits under "Other".
pub fn section_title(section: &Section) -> String {
    match section {
        Section::LastUsed => source_display_name("last_used"),
        Section::Pinned => source_display_name("pinned"),
        Section::Language(lang) if lang.is_empty() => source_display_name("other"),
        Section::Language(lang) => localized_display_name(lang),
    }
}

/// The language line of a row. A row in its own language group does not repeat it; the Last used and Pinned groups
/// mix languages, so their rows show it.
pub fn language_line(section: &Section, source: &Source) -> Option<String> {
    match section {
        Section::Language(_) => None,
        Section::LastUsed | Section::Pinned if source.lang.is_empty() => None,
        Section::LastUsed | Section::Pinned => Some(localized_display_name(&source.lang)),
    }
}

#[cfg(test)]
/// The row's pin button reads the `Pinned` bit: the last-used copy of a pinned source keeps it.
pub fn is_pinned(source: &Source) -> bool {
    source.pin.contains(Pin::Pinned)
}

#[cfg(test)]
/// The dimmed alpha of a secondary control (`SECONDARY_ALPHA` of presentation-core `Constants.kt`).
pub const SECONDARY_ALPHA: f64 = 0.78;

#[cfg(test)]
/// The pin button: filled at full strength when pinned, outline dimmed to `SECONDARY_ALPHA` otherwise (U§3.2).
pub fn pin_button_style(pinned: bool) -> (bool, f64) {
    if pinned { (true, 1.0) } else { (false, SECONDARY_ALPHA) }
}

/// The Latest button only appears when the source supports latest (E-SL-07).
pub fn shows_latest(source: &Source) -> bool {
    source.supports_latest
}

#[cfg(test)]
/// The disable toggle is not offered for the local source.
pub fn can_disable(id: i64) -> bool {
    id != LOCAL_SOURCE_ID
}

#[cfg(test)]
/// Rows of the source options dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsAction {
    Pin,
    Unpin,
    Disable,
}

#[cfg(test)]
/// The options dialog of a source: pin or unpin by `Pin.Pinned`, and disable when the source may be disabled (B§2.5).
pub fn options_actions(source: &Source) -> Vec<OptionsAction> {
    let mut actions = vec![if source.pin.contains(Pin::Pinned) { OptionsAction::Unpin } else { OptionsAction::Pin }];
    if can_disable(source.id) {
        actions.push(OptionsAction::Disable);
    }
    actions
}

#[cfg(test)]
/// The label of a row of the options popover.
pub fn options_label(action: OptionsAction) -> String {
    use kuro_i18n::{MsgKey, tr};
    tr(match action {
        OptionsAction::Pin => MsgKey::action_pin,
        OptionsAction::Unpin => MsgKey::action_unpin,
        OptionsAction::Disable => MsgKey::action_disable,
    })
}

#[cfg(test)]
/// The title of the options popover: the source's `visualName` (`name (LANG)`).
pub fn options_title(source: &Source) -> String {
    source.visual_name()
}

#[cfg(test)]
/// What choosing a row does: flip the source's pin, or its disabled flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsEffect {
    TogglePin,
    ToggleDisabled,
}

#[cfg(test)]
pub fn options_effect(action: OptionsAction) -> OptionsEffect {
    match action {
        OptionsAction::Pin | OptionsAction::Unpin => OptionsEffect::TogglePin,
        OptionsAction::Disable => OptionsEffect::ToggleDisabled,
    }
}

#[cfg(test)]
/// The two stored sets the toggles write, and the lock that orders them (E-SL-09).
pub struct SourceToggles {
    lock: Mutex<()>,
    pinned: Pref<BTreeSet<String>>,
    disabled: Pref<BTreeSet<String>>,
}

#[cfg(test)]
impl SourceToggles {
    pub fn new(pinned: Pref<BTreeSet<String>>, disabled: Pref<BTreeSet<String>>) -> Self {
        Self { lock: Mutex::new(()), pinned, disabled }
    }

    pub fn toggle_pin(&self, id: i64) -> Result<bool, PrefError> {
        toggle_pin_serialized(&self.lock, &self.pinned, id)
    }

    pub fn toggle_disabled(&self, id: i64) -> Result<bool, PrefError> {
        toggle_disabled_serialized(&self.lock, &self.disabled, id)
    }

    /// Runs the effect of an options row for source `id`.
    pub fn apply(&self, effect: OptionsEffect, id: i64) -> Result<bool, PrefError> {
        match effect {
            OptionsEffect::TogglePin => self.toggle_pin(id),
            OptionsEffect::ToggleDisabled => self.toggle_disabled(id),
        }
    }
}

#[cfg(test)]
/// Toggles one source's pin under a lock, so two quick toggles of different sources both persist (E-SL-09).
pub fn toggle_pin_serialized(lock: &Mutex<()>, pinned: &Pref<BTreeSet<String>>, id: i64) -> Result<bool, PrefError> {
    let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut set = pinned.get();
    let now_pinned = toggle_pin(&mut set, id);
    pinned.set(set)?;
    Ok(now_pinned)
}

#[cfg(test)]
/// Toggles one source's disabled flag under the same lock (`ToggleMangaSource`).
pub fn toggle_disabled_serialized(
    lock: &Mutex<()>,
    disabled: &Pref<BTreeSet<String>>,
    id: i64,
) -> Result<bool, PrefError> {
    let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut set = disabled.get();
    let now_disabled = flip_disabled(&mut set, id);
    disabled.set(set)?;
    Ok(now_disabled)
}

#[cfg(test)]
/// The toast of a failed source-list subscription. The last list stays on screen (E-SL-10).
pub fn list_failure_toast() -> ToastSpec {
    ToastSpec {
        message: kuro_i18n::tr(kuro_i18n::MsgKey::internal_error),
        action: None,
        duration: ToastDuration::Seconds(3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    fn src(id: i64, name: &str, lang: &str, supports_latest: bool) -> Source {
        let mut s = Source::new(id, name, lang);
        s.supports_latest = supports_latest;
        s
    }

    fn ids(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Fixture: 2 is pinned (lang "all"), 5 is disabled, 6 is in a language that is not enabled, 4 has no language,
    /// and 0 is the local source.
    fn fixture() -> Vec<Source> {
        vec![
            src(1, "Beta", "en", true),
            src(2, "alpha", "all", false),
            src(3, "Gamma", "de", false),
            src(4, "Delta", "", false),
            src(5, "Epsilon", "de", false),
            src(6, "Zeta", "ja", false),
            src(7, "Eta", "en", true),
            src(0, "Local", "other", false),
        ]
    }

    fn build(last_used: i64, enabled: &[&str], disabled: &[&str]) -> Vec<SourceRow> {
        let all = fixture();
        let enabled = ids(enabled);
        let disabled = ids(disabled);
        let pinned = ids(&["2"]);
        source_list(&ListInput { sources: &all, enabled_languages: &enabled, disabled: &disabled, pinned: &pinned, last_used })
    }

    fn headers(rows: &[SourceRow]) -> Vec<Section> {
        rows.iter()
            .filter_map(|r| match r {
                SourceRow::Header(s) => Some(s.clone()),
                SourceRow::Item(_) => None,
            })
            .collect()
    }

    fn item_ids(rows: &[SourceRow]) -> Vec<i64> {
        rows.iter()
            .filter_map(|r| match r {
                SourceRow::Item(s) => Some(s.id),
                SourceRow::Header(_) => None,
            })
            .collect()
    }

    #[test]
    fn sections_run_last_used_pinned_then_codes_with_empty_last() {
        let rows = build(7, &["en", "de", "other", "", "all"], &["5"]);
        assert_eq!(
            headers(&rows),
            vec![
                Section::LastUsed,
                Section::Pinned,
                Section::Language("de".into()),
                Section::Language("en".into()),
                Section::Language("other".into()),
                Section::Language(String::new()),
            ]
        );
    }

    #[test]
    fn items_are_sorted_by_name_within_their_section() {
        let rows = build(-1, &["en", "de", "other", "", "all"], &["5"]);
        // Pinned alpha (2), then de Gamma (3), en Beta (1) and Eta (7), other Local (0), unknown language Delta (4).
        assert_eq!(item_ids(&rows), vec![2, 3, 1, 7, 0, 4]);
    }

    #[test]
    fn last_used_source_appears_twice_with_the_copy_under_last_used() {
        let rows = build(7, &["en", "de", "other", "", "all"], &[]);
        assert_eq!(item_ids(&rows).iter().filter(|id| **id == 7).count(), 2);
        assert_eq!(rows[0], SourceRow::Header(Section::LastUsed));
        match &rows[1] {
            SourceRow::Item(s) => assert!(s.is_used_last),
            other => panic!("expected the copy, got {other:?}"),
        }
    }

    #[test]
    fn hidden_languages_and_disabled_sources_are_dropped() {
        let rows = build(-1, &["en"], &["5"]);
        let shown = item_ids(&rows);
        assert!(!shown.contains(&6), "ja is not enabled");
        assert!(!shown.contains(&5), "5 is disabled");
        assert!(!shown.contains(&3), "de is not enabled");
    }

    #[test]
    fn a_disabled_last_used_source_leaves_no_copy_and_no_header() {
        let rows = build(5, &["en", "de", "other", "", "all"], &["5"]);
        assert!(!headers(&rows).contains(&Section::LastUsed));
        assert!(!item_ids(&rows).contains(&5));
    }

    #[test]
    fn the_local_source_is_kept_even_when_its_language_is_not_enabled() {
        let rows = build(-1, &["en"], &[]);
        assert!(item_ids(&rows).contains(&0));
    }

    #[test]
    fn options_offer_pin_or_unpin_and_disable_except_for_local() {
        let mut pinned = src(3, "Gamma", "de", false);
        pinned.pin = Pins::PINNED;
        let mut copy = src(7, "Eta", "en", true);
        copy.pin = Pins::PINNED.minus(Pin::Actual);
        assert_eq!(options_actions(&src(3, "Gamma", "de", false)), vec![OptionsAction::Pin, OptionsAction::Disable]);
        assert_eq!(options_actions(&pinned), vec![OptionsAction::Unpin, OptionsAction::Disable]);
        assert_eq!(options_actions(&copy), vec![OptionsAction::Unpin, OptionsAction::Disable]);
        assert_eq!(options_actions(&src(0, "Local", "other", false)), vec![OptionsAction::Pin]);
        assert!(!can_disable(LOCAL_SOURCE_ID));
    }

    #[test]
    fn latest_button_follows_supports_latest_and_the_pin_button_styles_differ() {
        assert!(shows_latest(&src(1, "Beta", "en", true)));
        assert!(!shows_latest(&src(1, "Beta", "en", false)));
        assert_eq!(pin_button_style(true), (true, 1.0));
        assert_eq!(pin_button_style(false), (false, 0.78));
    }

    #[test]
    fn the_options_popover_is_titled_with_the_visual_name_and_each_row_maps_to_its_toggle() {
        let source = Source::new(7, "Example", "en");
        assert_eq!(options_title(&source), "Example (EN)");
        assert_eq!(options_title(&Source::new(7, "Example", "")), "Example");
        assert_eq!(options_effect(OptionsAction::Pin), OptionsEffect::TogglePin);
        assert_eq!(options_effect(OptionsAction::Unpin), OptionsEffect::TogglePin);
        assert_eq!(options_effect(OptionsAction::Disable), OptionsEffect::ToggleDisabled);
        for action in [OptionsAction::Pin, OptionsAction::Unpin, OptionsAction::Disable] {
            assert!(!options_label(action).is_empty());
        }
    }

    #[test]
    fn the_toggles_write_the_stored_sets_through_the_one_lock() {
        let store = PrefStore::in_memory();
        let pinned = store.pref("pinned", BTreeSet::<String>::new());
        let disabled = store.pref("hidden", BTreeSet::<String>::new());
        let toggles = SourceToggles::new(pinned.clone(), disabled.clone());
        assert!(toggles.apply(OptionsEffect::TogglePin, 5).expect("pin"));
        assert!(toggles.apply(OptionsEffect::ToggleDisabled, 5).expect("disable"));
        assert!(pinned.get().contains("5") && disabled.get().contains("5"));
        assert!(!toggles.apply(OptionsEffect::TogglePin, 5).expect("unpin"), "the second toggle undoes the first");
        assert!(!pinned.get().contains("5"));
    }

    #[test]
    fn two_quick_toggles_of_different_sources_both_persist() {
        let store = PrefStore::in_memory();
        let lock = Mutex::new(());
        let key = "pinned_catalogues";
        std::thread::scope(|scope| {
            for id in [11_i64, 22] {
                let store = store.clone();
                let lock = &lock;
                scope.spawn(move || {
                    let pinned = store.pref::<BTreeSet<String>>(key, BTreeSet::new());
                    toggle_pin_serialized(lock, &pinned, id).expect("toggle");
                });
            }
        });
        let stored = store.pref::<BTreeSet<String>>(key, BTreeSet::new()).get();
        assert_eq!(stored, ids(&["11", "22"]));
    }

    #[test]
    fn toggling_twice_unpins_and_disables_are_reversible() {
        let store = PrefStore::in_memory();
        let lock = Mutex::new(());
        let pinned = store.pref::<BTreeSet<String>>("pinned_catalogues", BTreeSet::new());
        assert!(toggle_pin_serialized(&lock, &pinned, 9).expect("pin"));
        assert!(!toggle_pin_serialized(&lock, &pinned, 9).expect("unpin"));
        let disabled = store.pref::<BTreeSet<String>>("hidden_catalogues", BTreeSet::new());
        assert!(toggle_disabled_serialized(&lock, &disabled, 9).expect("disable"));
        assert!(!toggle_disabled_serialized(&lock, &disabled, 9).expect("enable"));
    }

    #[test]
    fn a_failed_list_shows_the_internal_error_toast_for_three_seconds() {
        let toast = list_failure_toast();
        assert_eq!(toast.message, kuro_i18n::tr(kuro_i18n::MsgKey::internal_error));
        assert_eq!(toast.duration, ToastDuration::Seconds(3));
        assert_eq!(toast.action, None);
    }

    #[test]
    fn section_titles_use_the_four_special_names_and_the_language_name() {
        assert_eq!(section_title(&Section::LastUsed), "Last used");
        assert_eq!(section_title(&Section::Pinned), "Pinned");
        assert_eq!(section_title(&Section::Language(String::new())), "Other");
        assert_eq!(section_title(&Section::Language("all".into())), "Multi");
        assert_eq!(section_title(&Section::Language("zh-CN".into())), localized_display_name("zh-Hans"));
        assert_ne!(section_title(&Section::Language("en".into())), "en");
    }

    #[test]
    fn only_the_mixed_language_sections_repeat_the_language_on_a_row() {
        let source = src(1, "A", "en", false);
        assert_eq!(language_line(&Section::Language("en".into()), &source), None);
        assert_eq!(language_line(&Section::Pinned, &source), Some(localized_display_name("en")));
        assert_eq!(language_line(&Section::LastUsed, &source), Some(localized_display_name("en")));
        assert_eq!(language_line(&Section::Pinned, &src(2, "B", "", false)), None);
    }

    #[test]
    fn a_pinned_source_and_its_last_used_copy_both_read_as_pinned() {
        let mut sources = vec![src(1, "A", "en", false)];
        sources[0].pin = Pins::UNPINNED;
        let pinned: BTreeSet<String> = ["1".to_owned()].into();
        let languages: BTreeSet<String> = ["en".to_owned()].into();
        let rows = source_list(&ListInput {
            sources: &sources,
            enabled_languages: &languages,
            disabled: &BTreeSet::new(),
            pinned: &pinned,
            last_used: 1,
        });
        let items: Vec<&Source> = rows.iter().filter_map(|r| if let SourceRow::Item(s) = r { Some(s) } else { None }).collect();
        assert_eq!(items.len(), 2, "one under Last used, one under Pinned");
        assert!(items.iter().all(|s| is_pinned(s)));
        assert!(!is_pinned(&src(2, "B", "en", false)));
    }
}
