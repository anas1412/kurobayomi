//! The extensions tab: query filter, grouping, row warnings and actions, the install state machine, update-all, refresh
//! and the uninstall pop (T-40a-017, T-40a-018, T-40a-024). Ports `MangaExtensionsScreenModel`.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use kuro_extensions::InstallStep;
use kuro_i18n::{MsgKey, tr};

use super::names::{case_insensitive_cmp, compare_languages, localized_display_name};

/// A source as the extensions tab reads it. `base_url` is set only for HTTP sources, so a non-HTTP installed source is
/// never matched on its URL (E-40-07).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtSource {
    pub id: i64,
    pub name: String,
    /// The source's own language; the per-source split of an available extension keys its rows on it.
    pub lang: String,
    pub base_url: Option<String>,
}

/// Where an extension stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtState {
    Installed { has_update: bool, obsolete: bool },
    Available,
    Untrusted,
}

/// One extension row's facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ext {
    pub name: String,
    pub pkg: String,
    pub version: String,
    /// `None` only for an untrusted extension; `Some("")` is an empty language.
    pub lang: Option<String>,
    pub nsfw: bool,
    pub sources: Vec<ExtSource>,
    pub state: ExtState,
    /// The icon the package ships: a PNG or the package file itself (its `icon.png` entry is read).
    pub icon_file: Option<String>,
}

/// The query: comma-separated terms, matched with OR. Each term is trimmed and matched case-insensitively against
/// names, base URLs of HTTP sources and source ids. An untrusted extension matches on its name only (E-40-09).
pub fn matches_query(query: &str, ext: &Ext) -> bool {
    if query.is_empty() {
        return true;
    }
    query.split(',').any(|raw| {
        let term = raw.trim();
        if term.is_empty() {
            return false;
        }
        let needle = term.to_lowercase();
        let name_hit = ext.name.to_lowercase().contains(&needle);
        match ext.state {
            ExtState::Untrusted => name_hit,
            ExtState::Installed { .. } | ExtState::Available => {
                let id = term.parse::<i64>().ok();
                let source_hit = |s: &ExtSource| {
                    s.name.to_lowercase().contains(&needle)
                        || id == Some(s.id)
                        || s.base_url.as_deref().is_some_and(|url| url.to_lowercase().contains(&needle))
                };
                name_hit || ext.sources.iter().any(source_hit)
            }
        }
    })
}

/// The group an extension row sits in. Languages are keyed by their code, not their name (E-40-14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupKey {
    Updates,
    Installed,
    Language(String),
}

/// One header and the indices (into the input slice) of its rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtGroup {
    pub key: GroupKey,
    pub rows: Vec<usize>,
}

/// `GetMangaExtensionsByType` step 3: an available extension is dropped when an installed or untrusted one has its
/// package. Each of its sources in an enabled language becomes a row of its own with the synthetic package
/// `<pkg>-<source id>`, the source's name and language. An extension without sources stays one row when its own
/// language is enabled. `enabled` of `None` means every language. Installed and untrusted entries pass through.
pub fn split_available(exts: &[Ext], enabled: Option<&BTreeSet<String>>) -> Vec<Ext> {
    let owned: BTreeSet<&str> = exts.iter().filter(|e| e.state != ExtState::Available).map(|e| e.pkg.as_str()).collect();
    let on = |lang: &str| enabled.is_none_or(|set| set.contains(lang));
    let mut out = Vec::with_capacity(exts.len());
    for ext in exts {
        if ext.state != ExtState::Available {
            out.push(ext.clone());
        } else if owned.contains(ext.pkg.as_str()) {
            continue;
        } else if ext.sources.is_empty() {
            if on(ext.lang.as_deref().unwrap_or_default()) {
                out.push(ext.clone());
            }
        } else {
            for source in ext.sources.iter().filter(|s| on(&s.lang)) {
                out.push(Ext {
                    name: source.name.clone(),
                    pkg: format!("{}-{}", ext.pkg, source.id),
                    lang: Some(source.lang.clone()),
                    sources: vec![source.clone()],
                    ..ext.clone()
                });
            }
        }
    }
    out
}

/// The package an install is asked for: a synthetic `<pkg>-<source id>` row key maps back to its real package.
pub fn real_pkg(exts: &[Ext], row_pkg: &str) -> String {
    exts.iter()
        .find(|e| e.pkg == row_pkg && e.state == ExtState::Available)
        .and_then(|e| e.sources.first().and_then(|s| row_pkg.strip_suffix(&format!("-{}", s.id))))
        .unwrap_or(row_pkg)
        .to_owned()
}

/// Groups in order: updates pending (only when some exist), installed (those without an update) with untrusted after
/// it, then one group per language in `compare_languages` order. Updates and installed are sorted obsolete first, then
/// by name ignoring case; untrusted and available rows by name ignoring case. Empty groups are left out. With
/// `show_nsfw` off, NSFW installed and available entries are left out. Feed it `split_available` rows.
pub fn group(exts: &[Ext], query: &str, show_nsfw: bool) -> Vec<ExtGroup> {
    // The NSFW setting hides installed and available entries only; an untrusted one always shows (T3).
    let hit = |e: &Ext| matches_query(query, e) && (show_nsfw || !e.nsfw || e.state == ExtState::Untrusted);
    let pick = |want: &dyn Fn(&Ext) -> bool| -> Vec<usize> {
        let mut rows: Vec<usize> = exts.iter().enumerate().filter(|(_, e)| want(e) && hit(e)).map(|(i, _)| i).collect();
        rows.sort_by(|&a, &b| {
            let obsolete = |e: &Ext| matches!(e.state, ExtState::Installed { obsolete: true, .. });
            (!obsolete(&exts[a])).cmp(&!obsolete(&exts[b])).then_with(|| case_insensitive_cmp(&exts[a].name, &exts[b].name))
        });
        rows
    };
    let mut groups = Vec::new();

    let updates = pick(&|e| matches!(e.state, ExtState::Installed { has_update: true, .. }));
    if !updates.is_empty() {
        groups.push(ExtGroup { key: GroupKey::Updates, rows: updates });
    }

    let mut installed = pick(&|e| matches!(e.state, ExtState::Installed { has_update: false, .. }));
    installed.extend(pick(&|e| e.state == ExtState::Untrusted));
    if !installed.is_empty() {
        groups.push(ExtGroup { key: GroupKey::Installed, rows: installed });
    }

    let mut by_lang: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for i in pick(&|e| e.state == ExtState::Available) {
        by_lang.entry(exts[i].lang.clone().unwrap_or_default()).or_default().push(i);
    }
    let mut langs: Vec<(String, Vec<usize>)> = by_lang.into_iter().collect();
    langs.sort_by(|(a, _), (b, _)| compare_languages(a, b));
    groups.extend(langs.into_iter().map(|(lang, rows)| ExtGroup { key: GroupKey::Language(lang), rows }));

    groups
}

/// Header text of a language group.
pub fn language_header(lang: &str) -> String {
    localized_display_name(lang)
}

/// Why the list is empty. The repos button is offered in both cases (B§5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtEmpty {
    /// Nothing matched a non-empty query: `no_results_found`.
    NoResults,
    /// No extensions at all: `empty_screen`.
    Nothing,
}

pub fn empty_state(query: &str, has_rows: bool) -> Option<ExtEmpty> {
    match (has_rows, query.is_empty()) {
        (true, _) => None,
        (false, false) => Some(ExtEmpty::NoResults),
        (false, true) => Some(ExtEmpty::Nothing),
    }
}

/// The warning shown on a row. The first that applies wins: untrusted, then obsolete, then NSFW (E-40-19, E-40-20).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Warning {
    Untrusted,
    Obsolete,
    Nsfw,
}

pub fn warning(ext: &Ext) -> Option<Warning> {
    match ext.state {
        ExtState::Untrusted => Some(Warning::Untrusted),
        ExtState::Installed { obsolete: true, .. } => Some(Warning::Obsolete),
        _ if ext.nsfw => Some(Warning::Nsfw),
        _ => None,
    }
}

/// The language line of a row: installed rows show it only when it is not empty (E-40-17), untrusted rows never.
pub fn language_line(ext: &Ext) -> Option<String> {
    match (&ext.state, ext.lang.as_deref()) {
        (ExtState::Untrusted, _) | (_, None) | (_, Some("")) => None,
        (_, Some(lang)) => Some(localized_display_name(lang)),
    }
}

/// The version line, hidden when the version is empty (E-40-18).
pub fn version_line(ext: &Ext) -> Option<&str> {
    (!ext.version.is_empty()).then_some(ext.version.as_str())
}

/// The status text for a busy install. Completed and error steps show none (E-40-21, E-40-22).
pub fn status_key(step: InstallStep) -> Option<&'static str> {
    match step {
        InstallStep::Pending => Some("ext_pending"),
        InstallStep::Downloading => Some("ext_downloading"),
        InstallStep::Installing => Some("ext_installing"),
        InstallStep::Idle | InstallStep::Installed | InstallStep::Error => None,
    }
}

/// The buttons of a row (U§7.2). An install that just completed shows none (E-40-21).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowAction {
    Cancel,
    Retry,
    Settings,
    Update,
    Trust,
    WebView,
    Install,
}

impl RowAction {
    /// The symbolic icon of the button (U§7.2).
    pub fn icon(self) -> &'static str {
        match self {
            RowAction::Cancel => "window-close-symbolic",
            RowAction::Retry => "view-refresh-symbolic",
            RowAction::Settings => "emblem-system-symbolic",
            RowAction::Update | RowAction::Install => "folder-download-symbolic",
            RowAction::Trust => "security-high-symbolic",
            RowAction::WebView => "web-browser-symbolic",
        }
    }
}

impl RowAction {
    /// The tooltip and accessible name of the button.
    pub fn label(self) -> String {
        tr(match self {
            RowAction::Cancel => MsgKey::action_cancel,
            RowAction::Retry => MsgKey::action_retry,
            RowAction::Settings => MsgKey::label_settings,
            RowAction::Update => MsgKey::ext_update,
            RowAction::Trust => MsgKey::ext_trust,
            RowAction::WebView => MsgKey::action_web_view,
            RowAction::Install => MsgKey::ext_install,
        })
    }
}

/// The text of a busy install step (`status_key` as a string of the current language).
pub fn status_text(step: InstallStep) -> Option<String> {
    status_key(step).map(|_| {
        tr(match step {
            InstallStep::Pending => MsgKey::ext_pending,
            InstallStep::Downloading => MsgKey::ext_downloading,
            _ => MsgKey::ext_installing,
        })
    })
}

/// The warning line of a row.
pub fn warning_text(warning: Warning) -> String {
    tr(match warning {
        Warning::Untrusted => MsgKey::ext_untrusted,
        Warning::Obsolete => MsgKey::ext_obsolete,
        Warning::Nsfw => MsgKey::ext_nsfw_warning,
    })
}

pub fn row_actions(ext: &Ext, step: InstallStep) -> Vec<RowAction> {
    if step == InstallStep::Installed {
        return Vec::new();
    }
    if !step.is_completed() {
        return vec![RowAction::Cancel];
    }
    if step == InstallStep::Error {
        return vec![RowAction::Retry];
    }
    match &ext.state {
        ExtState::Installed { has_update: true, .. } => vec![RowAction::Settings, RowAction::Update],
        ExtState::Installed { has_update: false, .. } => vec![RowAction::Settings],
        ExtState::Untrusted => vec![RowAction::Trust],
        ExtState::Available if ext.sources.is_empty() => vec![RowAction::Install],
        ExtState::Available => vec![RowAction::WebView, RowAction::Install],
    }
}

/// Install steps of every package being installed. Two jobs for one package share a count, so the first one to
/// finish does not clear a row that the second still shows (E-40-23, R-40-03).
#[derive(Debug, Default)]
pub struct Downloads {
    slots: BTreeMap<String, Slot>,
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    step: InstallStep,
    holders: u32,
}

impl Downloads {
    /// A job for `pkg` starts: its step is Pending until the job reports another.
    pub fn begin(&mut self, pkg: &str) {
        let slot = self.slots.entry(pkg.to_owned()).or_insert(Slot { step: InstallStep::Pending, holders: 0 });
        slot.holders += 1;
    }

    /// A job reports its step. Ignored when no job for `pkg` is running.
    pub fn set(&mut self, pkg: &str, step: InstallStep) {
        if let Some(slot) = self.slots.get_mut(pkg) {
            slot.step = step;
        }
    }

    /// A job ended, by completion or cancel. The entry goes once the last job for the package has ended.
    pub fn finish(&mut self, pkg: &str) {
        if let Some(slot) = self.slots.get_mut(pkg) {
            slot.holders = slot.holders.saturating_sub(1);
            if slot.holders == 0 {
                self.slots.remove(pkg);
            }
        }
    }

    /// Drops the package's entry whatever the job count, for a cancel or an uninstall.
    pub fn clear(&mut self, pkg: &str) {
        self.slots.remove(pkg);
    }

    pub fn step(&self, pkg: &str) -> InstallStep {
        self.slots.get(pkg).map_or(InstallStep::Idle, |slot| slot.step)
    }

    /// True while the package has a step that is not completed.
    pub fn busy(&self, pkg: &str) -> bool {
        !self.step(pkg).is_completed()
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

/// The installed extensions with an update, in list order. Update-all runs them one after another (B§5.6, E-40-26).
pub fn update_all_queue(exts: &[Ext]) -> Vec<usize> {
    exts.iter()
        .enumerate()
        .filter(|(_, e)| matches!(e.state, ExtState::Installed { has_update: true, .. }))
        .map(|(i, _)| i)
        .collect()
}

/// The details page leaves when its extension is no longer installed (B§7.5).
pub fn details_should_pop(installed_pkgs: &[&str], pkg: &str) -> bool {
    !installed_pkgs.contains(&pkg)
}

/// The refresh spinner stays for at least one second (B§5.5).
pub const MIN_REFRESH_MS: u64 = 1000;

pub fn refresh_remaining_ms(elapsed_ms: u64) -> u64 {
    MIN_REFRESH_MS.saturating_sub(elapsed_ms)
}

/// The banner after a failed fetch of the available extensions. The previous snapshot stays on screen and Retry is
/// offered (T-41a-013).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchBanner {
    pub message: String,
    pub retry: bool,
}

pub fn fetch_failed_banner() -> FetchBanner {
    FetchBanner { message: kuro_i18n::tr(kuro_i18n::MsgKey::extension_api_error), retry: true }
}

/// The refreshing flag. It is cleared when the guard drops, on every path, so the spinner always stops (E-40-28).
#[derive(Clone, Debug, Default)]
pub struct Refresh {
    running: Rc<Cell<bool>>,
}

/// Holds the refreshing flag. Dropping it resets the flag.
pub struct RefreshGuard {
    running: Rc<Cell<bool>>,
}

impl Refresh {
    pub fn begin(&self) -> RefreshGuard {
        self.running.set(true);
        RefreshGuard { running: Rc::clone(&self.running) }
    }

    pub fn is_running(&self) -> bool {
        self.running.get()
    }
}

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        self.running.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_fetch_offers_retry_with_the_fetch_message() {
        let banner = fetch_failed_banner();
        assert!(banner.retry);
        assert_eq!(banner.message, kuro_i18n::tr(kuro_i18n::MsgKey::extension_api_error));
    }

    fn src(id: i64, name: &str, url: Option<&str>) -> ExtSource {
        ExtSource { id, name: name.into(), lang: "en".into(), base_url: url.map(str::to_owned) }
    }

    fn ext(name: &str, state: ExtState, lang: Option<&str>, sources: Vec<ExtSource>) -> Ext {
        Ext {
            name: name.into(),
            pkg: format!("eu.example.{}", name.to_lowercase()),
            version: "1.4.13".into(),
            lang: lang.map(str::to_owned),
            nsfw: false,
            sources,
            state,
            icon_file: None,
        }
    }

    fn installed(name: &str, has_update: bool) -> Ext {
        ext(name, ExtState::Installed { has_update, obsolete: false }, Some("en"), vec![src(1, "Alpha", Some("https://a.example"))])
    }

    fn available(name: &str, lang: &str) -> Ext {
        ext(name, ExtState::Available, Some(lang), vec![src(9, "Gamma", Some("https://g.example"))])
    }

    #[test]
    fn an_empty_query_matches_every_extension() {
        assert!(matches_query("", &installed("Foo", false)));
    }

    #[test]
    fn terms_are_trimmed_and_or_joined_and_empty_terms_match_nothing() {
        let e = installed("Foo", false);
        assert!(matches_query("bar,foo", &e));
        assert!(matches_query("x,,foo", &e), "an empty term is skipped, the other term still counts");
        assert!(!matches_query("   ", &e), "a query of spaces matches nothing");
        assert!(matches_query("  FOO  ", &e));
    }

    #[test]
    fn a_numeric_term_matches_a_source_id_including_negative_ones() {
        let e = ext("Other", ExtState::Available, Some("en"), vec![src(123, "Delta", None), src(-5, "Local", None)]);
        assert!(matches_query("123", &e));
        assert!(matches_query("-5", &e));
        assert!(!matches_query("12", &e));
    }

    #[test]
    fn a_non_http_installed_source_is_not_matched_on_its_url() {
        let e = ext("Other", ExtState::Installed { has_update: false, obsolete: false }, Some("en"), vec![src(3, "Local", None)]);
        assert!(!matches_query("example", &e));
    }

    #[test]
    fn an_untrusted_extension_matches_its_name_only() {
        let e = ext("Secret", ExtState::Untrusted, None, vec![src(4, "Source", Some("https://x.example"))]);
        assert!(matches_query("secret", &e));
        assert!(!matches_query("example", &e));
        assert!(!matches_query("4", &e));
    }

    #[test]
    fn groups_run_updates_then_installed_then_languages_in_language_order() {
        let exts = vec![
            available("Zed", "de"),
            installed("Foo", true),
            available("Bar", "all"),
            ext("Trusted", ExtState::Untrusted, None, vec![]),
            installed("Baz", false),
            available("Qux", "en"),
        ];
        let groups = group(&exts, "", true);
        let keys: Vec<GroupKey> = groups.iter().map(|g| g.key.clone()).collect();
        assert_eq!(
            keys,
            vec![
                GroupKey::Updates,
                GroupKey::Installed,
                GroupKey::Language("all".into()),
                GroupKey::Language("de".into()),
                GroupKey::Language("en".into()),
            ]
        );
        assert_eq!(groups[1].rows, vec![4, 3], "an extension with an update is not repeated here; untrusted after installed");
    }

    #[test]
    fn an_extension_with_an_update_appears_in_updates_only() {
        let exts = vec![installed("Foo", true), installed("Bar", false)];
        let groups = group(&exts, "", true);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], ExtGroup { key: GroupKey::Updates, rows: vec![0] });
        assert_eq!(groups[1], ExtGroup { key: GroupKey::Installed, rows: vec![1] });
    }

    #[test]
    fn installed_sort_obsolete_first_then_by_name_and_untrusted_and_available_by_name() {
        let mut obsolete = installed("zulu", false);
        obsolete.state = ExtState::Installed { has_update: false, obsolete: true };
        let exts = vec![
            installed("beta", false),
            obsolete,
            installed("Alpha", false),
            ext("yankee", ExtState::Untrusted, None, vec![]),
            ext("Xray", ExtState::Untrusted, None, vec![]),
            available("walrus", "en"),
            available("Victor", "en"),
        ];
        let groups = group(&exts, "", true);
        assert_eq!(groups[0].rows, vec![1, 2, 0, 4, 3], "obsolete first, then Alpha, beta, then untrusted Xray, yankee");
        assert_eq!(groups[1].rows, vec![6, 5], "available by name inside the language");
    }

    #[test]
    fn an_available_extension_splits_into_one_row_per_enabled_source_with_a_synthetic_package() {
        let mut multi = available("Multi", "all");
        multi.sources = vec![
            ExtSource { id: 7, name: "Multi EN".into(), lang: "en".into(), base_url: None },
            ExtSource { id: 8, name: "Multi JA".into(), lang: "ja".into(), base_url: None },
        ];
        let exts = vec![multi, installed("Own", false)];
        let enabled: BTreeSet<String> = ["ja".to_owned()].into();
        let rows = split_available(&exts, Some(&enabled));
        assert_eq!(rows.len(), 2);
        let row = &rows[0];
        assert_eq!((row.name.as_str(), row.pkg.as_str(), row.lang.as_deref()), ("Multi JA", "eu.example.multi-8", Some("ja")));
        assert_eq!(row.sources.len(), 1);
        assert_eq!(split_available(&exts, None).len(), 3, "no language filter keeps both sources");
        assert_eq!(real_pkg(&rows, "eu.example.multi-8"), "eu.example.multi");
        assert_eq!(real_pkg(&rows, "eu.example.own"), "eu.example.own");
    }

    #[test]
    fn an_available_extension_that_is_installed_or_untrusted_is_dropped_and_a_sourceless_one_needs_its_language() {
        let mut same = available("Foo", "en");
        same.pkg = "eu.example.foo".into();
        let mut sourceless = ext("Bare", ExtState::Available, Some("de"), vec![]);
        sourceless.pkg = "eu.example.bare".into();
        let exts = vec![same, installed("Foo", false), ext("Secret", ExtState::Untrusted, None, vec![]), sourceless];
        let rows = split_available(&exts, Some(&["en".to_owned()].into()));
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Foo", "Secret"], "installed wins; the de row is not enabled");
        assert_eq!(split_available(&exts, None).len(), 3);
    }

    #[test]
    fn a_query_removes_rows_from_every_group_and_empty_groups_disappear() {
        let exts = vec![installed("Foo", true), available("Bar", "en")];
        let groups = group(&exts, "bar", true);
        assert_eq!(groups, vec![ExtGroup { key: GroupKey::Language("en".into()), rows: vec![1] }]);
        assert!(group(&exts, "nothing-matches", true).is_empty());
    }

    #[test]
    fn nsfw_hides_installed_and_available_but_never_untrusted() {
        let mut available = ext("A", ExtState::Available, Some("en"), vec![]);
        available.nsfw = true;
        let mut untrusted = ext("U", ExtState::Untrusted, None, vec![]);
        untrusted.nsfw = true;
        let exts = vec![available, untrusted];
        let hidden = group(&exts, "", false);
        assert_eq!(hidden.iter().flat_map(|g| g.rows.clone()).collect::<Vec<_>>(), vec![1], "only the untrusted row shows");
        assert_eq!(group(&exts, "", true).iter().map(|g| g.rows.len()).sum::<usize>(), 2);    }

    #[test]
    fn two_languages_with_one_name_stay_in_separate_groups() {
        let exts = vec![available("A", "zh-CN"), available("B", "zh-Hans")];
        let groups = group(&exts, "", true);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn the_empty_state_says_no_results_only_when_a_query_is_set() {
        assert_eq!(empty_state("x", false), Some(ExtEmpty::NoResults));
        assert_eq!(empty_state("", false), Some(ExtEmpty::Nothing));
        assert_eq!(empty_state("x", true), None);
    }

    #[test]
    fn untrusted_beats_nsfw_and_obsolete_beats_nsfw() {
        let mut untrusted = ext("U", ExtState::Untrusted, None, vec![]);
        untrusted.nsfw = true;
        assert_eq!(warning(&untrusted), Some(Warning::Untrusted));
        let mut obsolete = installed("O", false);
        obsolete.state = ExtState::Installed { has_update: false, obsolete: true };
        obsolete.nsfw = true;
        assert_eq!(warning(&obsolete), Some(Warning::Obsolete));
        let mut nsfw = available("N", "en");
        nsfw.nsfw = true;
        assert_eq!(warning(&nsfw), Some(Warning::Nsfw));
        assert_eq!(warning(&available("Plain", "en")), None);
    }

    #[test]
    fn language_and_version_lines_hide_when_empty_or_absent() {
        assert_eq!(language_line(&ext("U", ExtState::Untrusted, None, vec![])), None);
        assert_eq!(language_line(&installed("I", false)).as_deref(), Some("English"));
        let blank = ext("B", ExtState::Installed { has_update: false, obsolete: false }, Some(""), vec![]);
        assert_eq!(language_line(&blank), None);
        let mut versionless = installed("V", false);
        versionless.version.clear();
        assert_eq!(version_line(&versionless), None);
        assert_eq!(version_line(&installed("V", false)), Some("1.4.13"));
    }

    #[test]
    fn status_text_shows_only_for_busy_steps() {
        assert_eq!(status_key(InstallStep::Pending), Some("ext_pending"));
        assert_eq!(status_key(InstallStep::Downloading), Some("ext_downloading"));
        assert_eq!(status_key(InstallStep::Installing), Some("ext_installing"));
        assert_eq!(status_key(InstallStep::Installed), None);
        assert_eq!(status_key(InstallStep::Error), None);
    }

    #[test]
    fn row_buttons_follow_the_action_table() {
        let idle = InstallStep::Idle;
        assert_eq!(row_actions(&installed("I", true), idle), vec![RowAction::Settings, RowAction::Update]);
        assert_eq!(row_actions(&installed("I", false), idle), vec![RowAction::Settings]);
        assert_eq!(row_actions(&ext("U", ExtState::Untrusted, None, vec![]), idle), vec![RowAction::Trust]);
        assert_eq!(row_actions(&available("A", "en"), idle), vec![RowAction::WebView, RowAction::Install]);
        let no_sources = ext("A", ExtState::Available, Some("en"), vec![]);
        assert_eq!(row_actions(&no_sources, idle), vec![RowAction::Install]);
        assert_eq!(row_actions(&installed("I", true), InstallStep::Downloading), vec![RowAction::Cancel]);
        assert_eq!(row_actions(&installed("I", true), InstallStep::Error), vec![RowAction::Retry]);
        assert!(row_actions(&available("A", "en"), InstallStep::Installed).is_empty());
    }

    #[test]
    fn two_jobs_for_one_package_keep_the_row_busy_until_both_end() {
        let mut downloads = Downloads::default();
        downloads.begin("eu.example.foo");
        downloads.begin("eu.example.foo");
        downloads.set("eu.example.foo", InstallStep::Downloading);
        downloads.finish("eu.example.foo");
        assert!(downloads.busy("eu.example.foo"), "one job is still running");
        downloads.finish("eu.example.foo");
        assert!(!downloads.busy("eu.example.foo"));
        assert!(downloads.is_empty());
    }

    #[test]
    fn a_completed_or_cancelled_job_returns_the_row_to_idle() {
        let mut downloads = Downloads::default();
        downloads.begin("p");
        downloads.set("p", InstallStep::Installing);
        downloads.finish("p");
        assert_eq!(downloads.step("p"), InstallStep::Idle);
        downloads.set("p", InstallStep::Installed);
        assert_eq!(downloads.step("p"), InstallStep::Idle, "a step without a job is ignored");
    }

    #[test]
    fn update_all_visits_only_installed_extensions_with_an_update_in_order() {
        let exts = vec![installed("A", false), installed("B", true), available("C", "en"), installed("D", true)];
        assert_eq!(update_all_queue(&exts), vec![1, 3]);
        assert!(update_all_queue(&[installed("A", false)]).is_empty());
    }

    #[test]
    fn details_pop_once_the_extension_is_gone() {
        assert!(!details_should_pop(&["eu.example.a", "eu.example.b"], "eu.example.a"));
        assert!(details_should_pop(&["eu.example.b"], "eu.example.a"));
    }

    #[test]
    fn refresh_holds_the_spinner_for_one_second_and_always_resets_it() {
        assert_eq!(refresh_remaining_ms(200), 800);
        assert_eq!(refresh_remaining_ms(5_000), 0);
        let refresh = Refresh::default();
        {
            let _guard = refresh.begin();
            assert!(refresh.is_running());
        }
        assert!(!refresh.is_running(), "the flag resets when the guard drops");
        let early = refresh.begin();
        drop(early);
        assert!(!refresh.is_running());
    }

    #[test]
    fn every_button_and_status_has_text() {
        for action in [
            RowAction::Cancel,
            RowAction::Retry,
            RowAction::Settings,
            RowAction::Update,
            RowAction::Trust,
            RowAction::WebView,
            RowAction::Install,
        ] {
            assert!(!action.label().is_empty(), "{action:?}");
        }
        for step in [InstallStep::Pending, InstallStep::Downloading, InstallStep::Installing] {
            assert!(status_text(step).is_some_and(|t| !t.is_empty()), "{step:?}");
        }
        for step in [InstallStep::Idle, InstallStep::Installed, InstallStep::Error] {
            assert_eq!(status_text(step), None, "{step:?}");
        }
        for warning in [Warning::Untrusted, Warning::Obsolete, Warning::Nsfw] {
            assert!(!warning_text(warning).is_empty());
        }
    }
}
