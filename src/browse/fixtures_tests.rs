//! The generic fixtures under `tests/fixtures/browse/` (T-40a-039) and the test-plan rows TC-40-101 to TC-40-121 that run
//! without GTK, each against a fake repository layer (a scripted `PageLoader`, the extension state machines and in-memory
//! preferences). Names are generic: no real titles, packages or covers.
//!
//! | Row | Where |
//! |---|---|
//! | TC-40-101, 102 | here, over a scripted `PageLoader` and `Listing` |
//! | TC-40-103 | here, over the fixtures |
//! | TC-40-104, 105 | here, over `Installs` |
//! | TC-40-106, 107, 108 | here, over `trust` and the long-press rule |
//! | TC-40-109, 110 | here, over `ext_details` and `Installs` |
//! | TC-40-111 | here, over the stored languages |
//! | TC-40-112, 113 | not built: the migrate dialog and its engine are not in `src/browse` (T-40a-015, T-40a-016), so no code exists to test |
//! | TC-40-114, 115 | here, over `migrate_nav`; its widget check is `migrate_nav::tests::smoke` |
//! | TC-40-116, 117 | here |
//! | TC-40-118 | manual: the key map needs a window and a real key press (`p1-ui-parity.md` section 9) |
//! | TC-40-119 | manual: the 360 px layout needs a real window; no automated check exists |
//! | TC-40-120 | here, over a settings file in the Aniyomi key names |
//! | TC-40-121 | here, over the message keys of section 18 |

use std::collections::BTreeSet;

use kuro_domain::source::browse::BrowseQuery;
use kuro_domain::source::model::Source;
use kuro_domain::source::migration::sort_migration_sources;
use kuro_domain::source::toggles::toggle_language;
use kuro_extensions::InstallStep;
use kuro_prefs::library::LibraryDisplayMode;
use kuro_prefs::source::{MigrationDirection, MigrationSortingMode, SourcePrefs};
use kuro_prefs::store::PrefStore;
use kuro_source::filter_list::FilterList;
use serde_json::Value;

use super::content::ContentPlan;
use super::ext_details::{DetailSource, detail_rows, set_all_enabled};
use super::extensions::{
    Ext, ExtLongPress, ExtSource, ExtState, GroupKey, RowAction, details_should_pop, ext_long_press, group, remove_answer, remove_confirm_spec,
    row_actions, split_available, status_text,
};
use super::global_search::{Candidate, GlobalSearch};
use super::installs::{Effect, Event, Input, Installs};
use super::lang_pages::sources_filter_groups;
use super::listing::{ListItem, ListedPage, Listing, LoadFuture, PageLoader};
use super::names::compare_languages;
use super::pages::{SourceFilter, progress_visible};
use super::primitives::{EmptyAction, ToastAction, ToastDuration, error_actions};
use super::trust::{TrustAnswer, TrustEffect, answer as trust_answer};

const REPO: &str = include_str!("../../tests/fixtures/browse/extensions_repo.json");
const INSTALLED: &str = include_str!("../../tests/fixtures/browse/installed_extensions.json");
const SOURCES: &str = include_str!("../../tests/fixtures/browse/sources.json");
const FAVORITES: &str = include_str!("../../tests/fixtures/browse/manga_favorites.json");
const CHAPTERS_OLD: &str = include_str!("../../tests/fixtures/browse/chapters_old.json");
const CHAPTERS_NEW: &str = include_str!("../../tests/fixtures/browse/chapters_new.json");
const TRACKS: &str = include_str!("../../tests/fixtures/browse/tracks.json");

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("a browse fixture is JSON")
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn ext_sources(value: &Value) -> Vec<ExtSource> {
    value["sources"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|s| ExtSource {
                    id: s["id"].as_i64().expect("source id"),
                    name: text(s, "name"),
                    lang: text(s, "lang"),
                    base_url: s["base_url"].as_str().map(str::to_owned),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extension(value: &Value, lang: Option<&str>, state: ExtState) -> Ext {
    Ext {
        name: text(value, "name"),
        pkg: text(value, "pkg"),
        version: text(value, "version"),
        lang: lang.map(str::to_owned),
        nsfw: value["nsfw"].as_bool().unwrap_or(false),
        sources: ext_sources(value),
        state,
        icon_file: value["icon"].as_str().map(str::to_owned),
    }
}

/// The sources of `sources.json` as the registry would hand them over.
fn sources_fixture() -> Vec<Source> {
    json(SOURCES)["sources"]
        .as_array()
        .expect("sources")
        .iter()
        .map(|s| {
            let mut source = Source::new(s["id"].as_i64().expect("id"), &text(s, "name"), &text(s, "lang"));
            source.supports_latest = s["supports_latest"].as_bool().unwrap_or(false);
            source.is_stub = s["is_stub"].as_bool().unwrap_or(false);
            if s["pinned"].as_bool().unwrap_or(false) {
                source.pin = kuro_domain::source::pin::Pins::PINNED;
            }
            source
        })
        .collect()
}

#[test]
fn every_fixture_parses_and_none_of_them_names_a_real_title_or_cover() {
    for (name, body) in [
        ("extensions_repo", REPO),
        ("installed_extensions", INSTALLED),
        ("sources", SOURCES),
        ("manga_favorites", FAVORITES),
        ("chapters_old", CHAPTERS_OLD),
        ("chapters_new", CHAPTERS_NEW),
        ("tracks", TRACKS),
    ] {
        let value = json(body);
        assert!(value["note"].as_str().is_some_and(|note| note.contains("Generic") || note.contains("generic")), "{name} says it is generic");
        assert!(!body.contains(".jpg") && !body.contains(".webp"), "{name} carries no cover image");
    }
}

#[test]
fn the_repo_fixture_has_the_required_edge_cases() {
    let repo = json(REPO);
    let list = repo["extensions"].as_array().expect("extensions");
    assert_eq!(list.len(), 6, "six extensions");
    let langs: BTreeSet<String> = list.iter().map(|e| text(e, "lang")).filter(|l| !l.is_empty()).collect();
    assert_eq!(langs, set(&["de", "en", "ja"]), "three languages");
    assert_eq!(list.iter().filter(|e| e["nsfw"].as_bool() == Some(true)).count(), 1, "one NSFW");
    assert_eq!(list.iter().filter(|e| e["obsolete"].as_bool() == Some(true)).count(), 1, "one obsolete");
    assert_eq!(list.iter().filter(|e| e["sources"].as_array().is_some_and(Vec::is_empty)).count(), 1, "one without sources");
    assert_eq!(list.iter().filter(|e| text(e, "lang").is_empty()).count(), 1, "one with an empty language");
}

#[test]
fn tc_40_103_extensions_group_as_updates_installed_then_languages() {
    // Three languages, two installed (one with an update), one untrusted. The repo's alpha is installed at an older version.
    let repo = json(REPO);
    let installed = json(INSTALLED);
    let mut exts = Vec::new();
    for item in installed["installed"].as_array().expect("installed") {
        let repo_version = repo["extensions"]
            .as_array()
            .and_then(|list| list.iter().find(|e| e["pkg"] == item["pkg"]))
            .map(|e| text(e, "version"))
            .unwrap_or_default();
        let has_update = repo_version != text(item, "version");
        exts.push(extension(item, Some(&text(item, "lang")), ExtState::Installed { has_update, obsolete: false }));
    }
    for item in installed["untrusted"].as_array().expect("untrusted") {
        exts.push(extension(item, None, ExtState::Untrusted));
    }
    let installed_pkgs: BTreeSet<String> = exts.iter().map(|e| e.pkg.clone()).collect();
    for item in repo["extensions"].as_array().expect("extensions") {
        if !installed_pkgs.contains(&text(item, "pkg")) {
            exts.push(extension(item, Some(&text(item, "lang")), ExtState::Available));
        }
    }

    let keys: Vec<GroupKey> = group(&exts, "", true).into_iter().map(|g| g.key).collect();
    assert_eq!(keys[0], GroupKey::Updates, "the update group comes first");
    assert_eq!(keys[1], GroupKey::Installed, "then the installed group");
    let langs: Vec<String> = keys[2..]
        .iter()
        .filter_map(|key| match key {
            GroupKey::Language(lang) => Some(lang.clone()),
            _ => None,
        })
        .collect();
    let mut sorted = langs.clone();
    sorted.sort_by(|a, b| compare_languages(a, b));
    assert_eq!(langs, sorted, "languages in LocaleHelper order");
    for lang in ["de", "en", "ja"] {
        assert!(langs.iter().any(|l| l == lang), "{lang} has a group");
    }
}

#[test]
fn sources_fixture_lists_online_sources_only_with_enabled_languages_first() {
    let sources = sources_fixture();
    let groups = sources_filter_groups(&sources, &set(&["en", "ja", "all"]));
    let ids: Vec<i64> = groups.iter().flat_map(|g| g.sources.iter().map(|s| s.id)).collect();
    assert!(!ids.contains(&0), "the local source is not an online source (E-SF-04)");
    assert!(!ids.contains(&2002), "a stub is not an online source");
    assert!(ids.contains(&4001), "a blank name is still listed");
    let flags: Vec<bool> = groups.iter().map(|g| g.enabled).collect();
    assert!(flags.windows(2).all(|w| w[0] || !w[1]), "enabled languages come first: {flags:?}");
    let alpha = groups.iter().find(|g| g.lang == "en").expect("en group");
    assert!(alpha.sources.iter().any(|s| s.id == 1002), "no source is hidden per source: the language switch decides");
}

#[test]
fn migrate_order_puts_stubs_first_ascending_and_keeps_ties_in_input_order() {
    let list: Vec<(Source, i64)> = sources_fixture()
        .into_iter()
        .map(|s| {
            let count = json(SOURCES)["sources"]
                .as_array()
                .and_then(|all| all.iter().find(|x| x["id"].as_i64() == Some(s.id)))
                .and_then(|x| x["count"].as_i64())
                .unwrap_or(0);
            (s, count)
        })
        .collect();
    let names = |out: &[(Source, i64)]| out.iter().map(|(s, _)| s.id).collect::<Vec<i64>>();

    let alphabetical = sort_migration_sources(list.clone(), MigrationSortingMode::ALPHABETICAL, MigrationDirection::ASCENDING);
    assert_eq!(names(&alphabetical), [2002, 4001, 3001, 1002, 1001, 2001, 5001], "stub first, then names in primary order");
    assert!(!names(&alphabetical).contains(&0), "the local source is never listed");

    let total = sort_migration_sources(list.clone(), MigrationSortingMode::TOTAL, MigrationDirection::ASCENDING);
    assert_eq!(names(&total), [2002, 1002, 4001, 3001, 1001, 5001, 2001], "stub first, then the counts, ties in input order");

    let descending = sort_migration_sources(list, MigrationSortingMode::ALPHABETICAL, MigrationDirection::DESCENDING);
    assert_eq!(names(&descending), [5001, 2001, 1001, 1002, 3001, 4001, 2002], "descending reverses the whole order, stubs last");
}

#[test]
fn duplicate_titles_compare_with_unicode_lowercase_on_both_sides() {
    let favorites = json(FAVORITES);
    let titles: Vec<(i64, String)> = favorites["favorites"]
        .as_array()
        .expect("favorites")
        .iter()
        .map(|f| (f["id"].as_i64().expect("id"), text(f, "title")))
        .collect();
    let same = |needle: &str| -> Vec<i64> {
        let needle = needle.to_lowercase();
        titles.iter().filter(|(_, t)| t.to_lowercase() == needle).map(|(id, _)| *id).collect()
    };
    assert_eq!(same("Manga A"), [1, 2], "case does not matter");
    assert_eq!(same("ÉCLAIR"), [11, 12], "accented capitals match the lowercase form (B-EC-25)");
    assert!(same("").contains(&9), "an empty title is a title of its own");
}

#[test]
fn tc_40_116_the_progress_bar_shows_only_while_some_but_not_all_have_answered() {
    assert!(!progress_visible(0, 10), "nothing answered: hidden");
    assert!(progress_visible(3, 10), "3 of 10: shown");
    assert!(!progress_visible(10, 10), "all answered: hidden");
}

#[test]
fn tc_40_117_a_chip_change_without_a_query_leaves_the_results_unchanged() {
    let candidates: Vec<Candidate> = sources_fixture()
        .into_iter()
        .map(|source| Candidate { source, package: String::new() })
        .collect();
    let mut search = GlobalSearch::new(candidates, set(&["en", "ja", "all", "other", "de"]), BTreeSet::new(), None, false);
    assert_eq!(search.set_filter(SourceFilter::All), None, "no query: nothing is searched");
    assert!(search.rows().is_empty(), "the results are unchanged (empty)");
}

#[test]
fn a_chip_change_with_the_query_searches_only_the_sources_it_adds() {
    let candidates: Vec<Candidate> = sources_fixture()
        .into_iter()
        .map(|source| Candidate { source, package: String::new() })
        .collect();
    let mut search = GlobalSearch::new(candidates, set(&["en", "ja", "all", "other", "de"]), BTreeSet::new(), None, false);
    let first = search.request("title", None).expect("the first search runs the pinned source");
    assert_eq!(first.sources.iter().map(|s| s.id).collect::<Vec<_>>(), [1001]);
    search.apply(first.generation, 1001, Ok(Vec::new()));
    let added = search.set_filter(SourceFilter::All).expect("the same query searches the new sources");
    assert!(added.sources.iter().all(|s| s.id != 1001), "the answered pinned Alpha Source is not searched again");
    assert!(added.sources.iter().any(|s| s.id == 5001), "an unpinned source is added");
}

#[test]
fn the_migrate_and_chapter_fixtures_have_the_shape_the_edge_cases_name() {
    let old = json(CHAPTERS_OLD);
    let new = json(CHAPTERS_NEW);
    let old_list = old["chapters"].as_array().expect("chapters");
    assert!(old_list.iter().any(|c| c["number"].as_f64() == Some(10.5)), "old chapters read up to 10.5");
    assert_eq!(old_list.iter().filter(|c| c["number"].as_f64() == Some(-1.0)).count(), 1, "one unrecognised chapter");
    let new_list = new["chapters"].as_array().expect("chapters");
    assert_eq!(new_list.len(), 13, "chapters 1 to 12 and 5.5");
    let tracks = json(TRACKS);
    let matched = tracks["tracks"].as_array().expect("tracks").iter().filter(|t| t["matched_by_fake_tracker"].as_bool() == Some(true)).count();
    assert_eq!(matched, 1, "one track matched by the fake tracker");
}

#[test]
fn the_kurobayomi_strings_the_browse_pages_draw_resolve_to_text() {
    use kuro_i18n::{MsgKey, tr};
    for key in [MsgKey::ext_updates_pending, MsgKey::ext_installed, MsgKey::not_installed, MsgKey::internal_error, MsgKey::has_results] {
        let text = tr(key);
        assert!(!text.trim().is_empty(), "a browse string resolves to text");
    }
}

// ---------- the fake repository layer ----------

/// A source repository that answers from a script: each call to `load` takes the next answer, and an empty script
/// answers one fresh page.
struct FakeRepo {
    answers: std::sync::Mutex<Vec<Result<ListedPage, String>>>,
    asked: std::sync::Mutex<Vec<u32>>,
}

impl FakeRepo {
    fn new(answers: Vec<Result<ListedPage, String>>) -> Self {
        Self { answers: std::sync::Mutex::new(answers), asked: std::sync::Mutex::default() }
    }

    fn pages_asked(&self) -> Vec<u32> {
        self.asked.lock().expect("lock").clone()
    }
}

impl PageLoader for FakeRepo {
    fn load(&self, _query: BrowseQuery, _filters: FilterList, page: u32) -> LoadFuture {
        self.asked.lock().expect("lock").push(page);
        let next = {
            let mut answers = self.answers.lock().expect("lock");
            if answers.is_empty() { Ok(listed(&["fresh"], false)) } else { answers.remove(0) }
        };
        Box::pin(async move { next })
    }
}

fn listed(titles: &[&str], has_next: bool) -> ListedPage {
    ListedPage { items: titles.iter().map(|t| ListItem { title: (*t).into(), url: format!("/{t}"), ..ListItem::default() }).collect(), has_next }
}

/// Runs the repository's next page for the listing the way the page does: ask, load, deliver or fail.
fn step(listing: &mut Listing, repo: &FakeRepo) -> Option<usize> {
    let ask = listing.ask()?;
    let result = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(repo.load(listing.query().clone(), FilterList::default(), ask.page));
    match result {
        Ok(page) => listing.deliver(ask.generation, page),
        Err(message) => {
            listing.fail(ask.generation, message);
            Some(0)
        }
    }
}

fn plan(listing: &Listing) -> ContentPlan {
    listing.plan(true, false, false, LibraryDisplayMode::CompactGrid)
}

#[test]
fn tc_40_101_a_failing_source_shows_its_actions_in_order_and_retry_loads_the_first_page_again() {
    let repo = FakeRepo::new(vec![Err("offline".into())]);
    let mut listing = Listing::new(BrowseQuery::Popular);
    step(&mut listing, &repo);
    match plan(&listing) {
        ContentPlan::Error { message, actions } => {
            assert_eq!(message, "offline");
            assert_eq!(actions, error_actions(false), "the actions of a web source, in their order");
            assert_eq!(&actions[..2], [EmptyAction::Retry, EmptyAction::OpenInWebView], "Retry first, then Open in web view");
            assert!(actions.len() <= 3, "at most three actions");
        }
        other => panic!("an error state: {other:?}"),
    }
    listing.clear_errors();
    assert_eq!(step(&mut listing, &repo), Some(1), "retry asks again and the repository answers");
    assert_eq!(repo.pages_asked(), [1, 1], "the same first page twice");
    assert!(matches!(plan(&listing), ContentPlan::Items { toast: None, .. }));
}

#[test]
fn tc_40_102_a_failing_later_page_keeps_the_items_and_the_retry_toast_loads_that_page_again() {
    let repo = FakeRepo::new(vec![Ok(listed(&["a", "b"], true)), Err("page 2 failed".into())]);
    let mut listing = Listing::new(BrowseQuery::Popular);
    step(&mut listing, &repo);
    step(&mut listing, &repo);
    match plan(&listing) {
        ContentPlan::Items { toast: Some(toast), .. } => {
            assert_eq!(toast.message, "page 2 failed");
            assert_eq!(toast.action, Some(ToastAction::Retry), "the toast carries Retry");
            assert_eq!(toast.duration, ToastDuration::Indefinite, "and stays until it is answered");
        }
        other => panic!("items with a toast: {other:?}"),
    }
    assert_eq!(listing.loaded(), 2, "the items stay");
    listing.clear_errors();
    assert_eq!(step(&mut listing, &repo), Some(1));
    assert_eq!(repo.pages_asked(), [1, 2, 2], "retry asks page 2 again");
    assert_eq!(listing.loaded(), 3);
}

// ---------- the extension rows ----------

fn untrusted_fixture() -> Ext {
    let installed = json(INSTALLED);
    extension(&installed["untrusted"][0], None, ExtState::Untrusted)
}

fn installed_ext(pkg: &str) -> Ext {
    let installed = json(INSTALLED);
    let item = installed["installed"].as_array().expect("installed").iter().find(|e| e["pkg"] == pkg).expect("fixture package");
    extension(item, Some(&text(item, "lang")), ExtState::Installed { has_update: false, obsolete: false })
}

fn available_ext() -> Ext {
    let repo = json(REPO);
    let item = repo["extensions"].as_array().expect("extensions").iter().find(|e| e["sources"].as_array().is_some_and(|s| !s.is_empty())).expect("a repo entry with sources");
    extension(item, Some(&text(item, "lang")), ExtState::Available)
}

/// Applies the effects of an input and reports which jobs the page would start and abort.
fn started(effects: &[Effect]) -> Vec<(String, u64)> {
    effects.iter().filter_map(|e| if let Effect::Start { pkg, job } = e { Some((pkg.clone(), *job)) } else { None }).collect()
}

#[test]
fn tc_40_104_an_install_shows_a_busy_row_with_the_text_of_each_step_and_cancel_only_then_settings() {
    let ext = available_ext();
    let mut installs = Installs::default();
    let effects = installs.apply(Input::Row { pkg: ext.pkg.clone(), event: Event::Request });
    let (_, job) = started(&effects).remove(0);
    assert_eq!(installs.step(&ext.pkg), InstallStep::Pending);
    let mut texts = Vec::new();
    for step in [InstallStep::Pending, InstallStep::Downloading, InstallStep::Installing] {
        installs.apply(Input::Job { pkg: ext.pkg.clone(), job, event: Event::Report(step) });
        assert_eq!(installs.step(&ext.pkg), step);
        assert_eq!(row_actions(&ext, installs.step(&ext.pkg)), [RowAction::Cancel], "a busy row offers Cancel only");
        texts.push(status_text(step).expect("a busy step has a text"));
    }
    assert_eq!(texts.len(), 3);
    assert!(texts.iter().collect::<BTreeSet<_>>().len() == 3, "each step has its own text: {texts:?}");
    installs.apply(Input::Job { pkg: ext.pkg.clone(), job, event: Event::Report(InstallStep::Installed) });
    installs.apply(Input::Job { pkg: ext.pkg.clone(), job, event: Event::End });
    assert!(installs.is_empty(), "the finished job leaves nothing behind");
    let now_installed = Ext { state: ExtState::Installed { has_update: false, obsolete: false }, ..ext };
    assert_eq!(row_actions(&now_installed, installs.step(&now_installed.pkg)), [RowAction::Settings], "then the row offers Settings");
}

#[test]
fn tc_40_105_cancelling_an_install_aborts_the_job_returns_the_row_to_install_and_clears_the_map() {
    let ext = available_ext();
    let mut installs = Installs::default();
    let (_, job) = started(&installs.apply(Input::Row { pkg: ext.pkg.clone(), event: Event::Request })).remove(0);
    installs.apply(Input::Job { pkg: ext.pkg.clone(), job, event: Event::Report(InstallStep::Downloading) });
    let effects = installs.apply(Input::Row { pkg: ext.pkg.clone(), event: Event::Cancel });
    assert_eq!(effects, [Effect::Abort { pkg: ext.pkg.clone(), job }], "the running job is aborted");
    assert_eq!(installs.step(&ext.pkg), InstallStep::Idle);
    assert!(installs.is_empty(), "the per-package map is cleared");
    assert!(row_actions(&ext, InstallStep::Idle).contains(&RowAction::Install), "the row offers Install again");
    // a report of the aborted job that is still on its way changes nothing
    installs.apply(Input::Job { pkg: ext.pkg.clone(), job, event: Event::Report(InstallStep::Installing) });
    assert!(installs.is_empty());
}

/// The trusted set and the packages the host knows, as the trust flow of the extensions page reads and writes them.
struct FakeHost {
    prefs: SourcePrefs,
    untrusted: Vec<Ext>,
    removed: Vec<String>,
}

impl FakeHost {
    fn new(untrusted: Vec<Ext>) -> Self {
        Self { prefs: SourcePrefs::new(PrefStore::in_memory()), untrusted, removed: Vec::new() }
    }

    fn still_untrusted(&self, pkg: &str) -> bool {
        self.untrusted.iter().any(|e| e.pkg == pkg) && !self.prefs.trusted_extensions.get().contains(pkg)
    }

    /// A click on an untrusted row, answered with `chosen`.
    fn click(&mut self, pkg: &str, chosen: TrustAnswer) -> TrustEffect {
        let effect = trust_answer(chosen, self.still_untrusted(pkg));
        match effect {
            TrustEffect::Trust => {
                let mut trusted = self.prefs.trusted_extensions.get();
                trusted.insert(pkg.to_owned());
                self.prefs.trusted_extensions.set(trusted).expect("store");
            }
            TrustEffect::Uninstall => {
                self.untrusted.retain(|e| e.pkg != pkg);
                self.removed.push(pkg.to_owned());
            }
            TrustEffect::Nothing => {}
        }
        effect
    }
}

#[test]
fn tc_40_106_trusting_an_untrusted_extension_stores_its_package_and_it_becomes_installed() {
    let ext = untrusted_fixture();
    assert_eq!(row_actions(&ext, InstallStep::Idle), [RowAction::Trust], "the untrusted row's button is Trust");
    let mut host = FakeHost::new(vec![ext.clone()]);
    assert_eq!(host.click(&ext.pkg, TrustAnswer::Trust), TrustEffect::Trust);
    assert!(host.prefs.trusted_extensions.get().contains(&ext.pkg), "the package is stored as trusted");
    assert!(!host.still_untrusted(&ext.pkg), "it is no longer untrusted, so the host lists it as installed");
    let installed = Ext { state: ExtState::Installed { has_update: false, obsolete: false }, ..ext };
    let keys: Vec<GroupKey> = group(&[installed], "", true).into_iter().map(|g| g.key).collect();
    assert_eq!(keys, [GroupKey::Installed]);
    assert_eq!(host.click("eu.example.unknown", TrustAnswer::Trust), TrustEffect::Nothing, "a second answer finds nothing to trust");
}

#[test]
fn tc_40_107_uninstalling_from_the_trust_dialog_removes_the_extension() {
    let ext = untrusted_fixture();
    let mut host = FakeHost::new(vec![ext.clone()]);
    assert_eq!(host.click(&ext.pkg, TrustAnswer::Uninstall), TrustEffect::Uninstall);
    assert_eq!(host.removed, [ext.pkg.clone()]);
    assert!(host.untrusted.is_empty(), "the extension is gone");
    let mut installs = Installs::default();
    assert_eq!(installs.apply(Input::Row { pkg: ext.pkg.clone(), event: Event::Uninstall }), [Effect::Uninstall { pkg: ext.pkg }], "the page runs the uninstall");
}

#[test]
fn tc_40_108_a_private_extension_asks_before_it_is_removed_and_only_the_first_button_removes() {
    let ext = installed_ext("eu.example.gamma");
    assert_eq!(ext_long_press(&ext.state, false), ExtLongPress::ConfirmRemove, "no package directory: ask first");
    assert_eq!(ext_long_press(&ext.state, true), ExtLongPress::RemoveNow, "a package directory: remove at once");
    let dialog = remove_confirm_spec(&ext.name);
    assert!(dialog.body.as_deref().is_some_and(|body| body.contains("Gamma Reader")), "the body names the extension");
    assert_eq!(dialog.buttons.len(), 2, "Remove and Cancel");
    assert!(remove_answer(Some(0)), "the confirm removes");
    assert!(!remove_answer(Some(1)) && !remove_answer(None), "Cancel and a dismissed dialog remove nothing");
}

#[test]
fn tc_40_109_enable_all_turns_every_source_of_the_extension_on() {
    let installed = installed_ext("eu.example.alpha");
    let sources: Vec<DetailSource> = installed
        .sources
        .iter()
        .map(|s| DetailSource { id: s.id, name: s.name.clone(), lang: s.lang.clone(), label_as_name: false, configurable: false, base_url: s.base_url.clone() })
        .collect();
    let ids: Vec<i64> = sources.iter().map(|s| s.id).collect();
    let disabled: BTreeSet<String> = ids.iter().map(i64::to_string).chain(["999".to_owned()]).collect();
    assert!(detail_rows(&sources, &disabled).iter().all(|row| !row.row.enabled), "every source starts off");
    let after = set_all_enabled(&disabled, &ids, true);
    assert!(detail_rows(&sources, &after).iter().all(|row| row.row.enabled), "every source switch is on");
    assert_eq!(after, set(&["999"]), "another extension's disabled source is left alone");
}

#[test]
fn tc_40_110_the_details_page_pops_once_the_package_is_gone() {
    let ext = installed_ext("eu.example.gamma");
    let mut installs = Installs::default();
    let effects = installs.apply(Input::Row { pkg: ext.pkg.clone(), event: Event::Uninstall });
    assert_eq!(effects, [Effect::Uninstall { pkg: ext.pkg.clone() }]);
    assert!(!details_should_pop(&["eu.example.gamma", "eu.example.alpha"], &ext.pkg) , "the package is still installed: the page stays");
    assert!(details_should_pop(&["eu.example.alpha"], &ext.pkg), "the removal event lists no such package: the page pops");
}

#[test]
fn tc_40_111_toggling_a_language_saves_the_set_and_the_group_list_follows() {
    let prefs = SourcePrefs::new(PrefStore::in_memory());
    prefs.enabled_languages.set(set(&["en"])).expect("start");
    let repo = json(REPO);
    let exts: Vec<Ext> = repo["extensions"]
        .as_array()
        .expect("extensions")
        .iter()
        .map(|item| extension(item, Some(&text(item, "lang")), ExtState::Available))
        .collect();
    let languages = |enabled: &BTreeSet<String>| -> BTreeSet<String> {
        group(&split_available(&exts, Some(enabled)), "", true)
            .into_iter()
            .filter_map(|g| if let GroupKey::Language(lang) = g.key { Some(lang) } else { None })
            .collect()
    };
    assert_eq!(languages(&prefs.enabled_languages.get()), set(&["en"]), "only the enabled language has a group");
    let mut now = prefs.enabled_languages.get();
    assert!(toggle_language(&mut now, "ja"), "the toggle turns Japanese on");
    prefs.enabled_languages.set(now).expect("saved");
    assert_eq!(prefs.enabled_languages.get(), set(&["en", "ja"]), "the preference holds the new set");
    assert_eq!(languages(&prefs.enabled_languages.get()), set(&["en", "ja"]), "the group list follows the stored languages");
}

#[test]
fn tc_40_120_aniyomi_style_settings_in_a_toml_file_read_as_the_same_modes_and_sets() {
    use kuro_prefs::toml_backend::{TomlBackend, config_file};
    let dir = tempfile::tempdir().expect("tempdir");
    let (config, state) = (dir.path().join("config"), dir.path().join("state"));
    std::fs::create_dir_all(&config).expect("config dir");
    std::fs::create_dir_all(&state).expect("state dir");
    // the exporter writes each Aniyomi key under [sources] by its field name, with the Aniyomi spelling of the value
    std::fs::write(
        config_file(&config),
        "[sources]\ndisabled_manga_sources = [\"11\", \"12\"]\npinned_manga_sources = [\"5\"]\nenabled_languages = [\"en\", \"ja\"]\n",
    )
    .expect("seed");
    let backend = TomlBackend::open(&config, &state).expect("open");
    let store = PrefStore::new(Box::new(backend)).expect("store");
    let prefs = SourcePrefs::new(store);
    assert_eq!(prefs.disabled_manga_sources.key(), "hidden_catalogues");
    assert_eq!(prefs.pinned_manga_sources.key(), "pinned_catalogues");
    assert_eq!(prefs.disabled_manga_sources.get(), set(&["11", "12"]), "the hidden sources, as decimal ids");
    assert_eq!(prefs.pinned_manga_sources.get(), set(&["5"]), "the pinned sources");
    assert_eq!(prefs.enabled_languages.get(), set(&["en", "ja"]), "the enabled languages");
}

// ---------- TC-40-121: the message keys of section 18 ----------

/// Every key of section 18 of `p1-behavior.md`, as a `MsgKey`: a key that is not in the table does not compile.
fn section_18_keys() -> Vec<(&'static str, kuro_i18n::MsgKey)> {
    use kuro_i18n::MsgKey as K;
    macro_rules! keys {
        ($($name:ident),* $(,)?) => { vec![$((stringify!($name), K::$name)),*] };
    }
    keys![
        // dialogs and titles
        are_you_sure, remove_manga, action_remove, action_cancel, ext_confirm_remove, ext_remove, remove_private_extension_message,
        untrusted_extension, untrusted_extension_message, ext_trust, ext_uninstall, migration_dialog_what_to_include, copy, migrate,
        action_show_manga, ext_nsfw_warning, action_ok,
        // empty, error and loading
        empty_screen, no_results_found, unknown_error, internal_error, source_empty_screen, source_filter_empty_screen,
        information_empty_library, source_not_installed, local_source_help_guide, action_retry, action_open_in_web_view, label_help,
        // the browse toolbar
        action_display_mode, action_display_comfortable_grid, action_display_grid, action_display_list, action_settings, browse,
        // the sources list
        latest, action_pin, action_unpin, action_disable, not_installed, label_sources, pinned_sources, all, has_results,
        // the extension list
        label_manga_extensions, label_extensions, label_extension_repos, action_filter, ext_updates_pending, ext_installed,
        ext_update_all, ext_update, ext_install, ext_pending, ext_downloading, ext_installing, ext_untrusted, ext_obsolete,
        ext_nsfw_short, ext_permission_install_apps_warning,
        // the extension details
        label_extension_info, action_open_repo, action_enable_all, action_disable_all, pref_clear_cookies, obsolete_extension_message,
        ext_info_version, ext_info_language, ext_info_age_rating, ext_app_info, pref_incognito_mode, pref_incognito_mode_extension_summary,
        label_settings,
        // migration
        migration_selection_prompt, action_sort_alpha, action_sort_count, action_asc, action_desc, chapters, categories, custom_cover,
        delete_downloaded,
    ]
}

#[test]
fn tc_40_121_every_message_key_of_section_18_resolves_to_text_in_the_table() {
    for (name, key) in section_18_keys() {
        assert_eq!(key.id(), name, "the variant is the key");
        let shown = kuro_i18n::tr(key);
        assert!(!shown.trim().is_empty(), "{name} has text");
        assert_ne!(shown, name, "{name} resolves in the catalogue and is not shown as its own id");
    }
}

#[test]
fn tc_40_121_the_list_above_is_the_list_of_section_18_of_the_behavior_spec() {
    // the plan carries a filler string between characters in some checkouts; it is stripped before the section is read
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/plans/40-ui-browse-manga/p1-behavior.md");
    let Ok(raw) = std::fs::read_to_string(path) else { return };
    let plan = raw.replace("part 2 of this area (, IDs )", "");
    let section = plan.split("## 18.").nth(1).and_then(|rest| rest.split("\n## 19.").next()).expect("section 18");
    let mut in_plan = BTreeSet::new();
    let mut rest = section;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('`') else { break };
        let token = &after[..end];
        if !token.is_empty() && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            in_plan.insert(token.to_owned());
        }
        rest = &after[end + 1..];
    }
    let ours: BTreeSet<String> = section_18_keys().into_iter().map(|(name, _)| name.to_owned()).collect();
    let missing: Vec<&String> = in_plan.difference(&ours).collect();
    assert!(missing.is_empty(), "keys of section 18 that the check does not cover: {missing:?}");
    let extra: Vec<&String> = ours.difference(&in_plan).collect();
    assert!(extra.is_empty(), "keys the check names that section 18 does not: {extra:?}");
}
