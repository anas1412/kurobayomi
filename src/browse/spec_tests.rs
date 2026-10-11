//! The test plan of part 1 (T-41a-049, `p1-edge-cases-and-tests.md` sections 4 and 5): the fixtures checked in under
//! `tests/fixtures/browse_anime/`, the tests named after T-IDs that no other module of this lane carries, the integration
//! tests I1 to I4, and the string drift test over section 24 of the behavior spec. Every T-ID is accounted for in the
//! table below: it names the test that holds it, or the lane that owns the code it tests.
//!
//! | T-ID | Where |
//! |---|---|
//! | T1, T9, T10, T13 | here (over the fixtures) and `extensions::tests::an_available_extension_splits_...` |
//! | T2 | `extensions_page::tests::an_available_row_installs_the_real_package_of_its_synthetic_key` |
//! | T3 | `extensions::tests::nsfw_hides_installed_and_available_but_never_untrusted` and here |
//! | T4 | `extensions::tests::terms_are_trimmed_and_or_joined_and_empty_terms_match_nothing` |
//! | T5, T6, T7 | here (an `Ext` has no store name, so only names, ids and urls can match) |
//! | T11 | `extensions::tests::installed_sort_obsolete_first_then_by_name_...` |
//! | T14 | `extensions::tests::update_all_only_visible_set` |
//! | T15, T16, T17, T19 | `installs::tests` and `extensions_page::tests` |
//! | T18 | `shell::tests::enter_runs_a_global_search_of_the_visible_kind` (the extension filter is cut) |
//! | T20, T21, T22 | `sources::tests`, `header::tests` |
//! | T23 | `lang_pages::tests::the_language_switches_write_the_set_and_a_switched_off_language_has_no_list` |
//! | T24 | `ext_details::tests::the_debug_text_is_the_block_of_the_header_click` |
//! | T25, T27, T29, T38, T40, T41 | here |
//! | T26 | `extensions::tests::details_pop_once_the_extension_is_gone` |
//! | T28 | `trust::tests::trust_and_uninstall_act_only_while_the_entry_is_still_untrusted` |
//! | T39 | `ext_details_page::tests::the_incognito_switch_writes_the_package_into_the_set` |
//! | T42, T43, T44, T45, T49 | `primitives`, `sources` and `anime::sources` tests |
//! | T46, T47 | `prefs::tests`, `grid::tests` |
//! | T48 | `pages::pure_tests::the_progress_bar_shows_only_from_one_answer_until_all_have_answered` |
//! | T51, T53 | `grid::tests::a_stale_cover_never_reaches_a_recycled_cell`, `tasks::tests`, `glue::tests` |
//! | T52 | `source_prefs_page::pure_tests::the_summary_is_the_declared_one_else_the_current_value` |
//! | T50 (season list), T30 to T37, T54 to T56 | the migration lane: the season picker, `Migrate` and its flags are not built in `src/browse` |

use std::collections::BTreeSet;

use kuro_i18n::{Arg, MsgKey, tr, trf};
use serde_json::Value;

use super::ext_details::{DetailSource, clear_cookies, detail_rows};
use super::extensions::{Ext, ExtSource, ExtState, GroupKey, group, matches_query, split_available};
use super::names::{localized_display_name, subtitle_display_name};

/// Every fixture of section 5, checked in as JSON.
pub const FIXTURES: [(&str, &str); 11] = [
    ("ext_installed.json", include_str!("../../tests/fixtures/browse_anime/ext_installed.json")),
    ("ext_available.json", include_str!("../../tests/fixtures/browse_anime/ext_available.json")),
    ("ext_untrusted.json", include_str!("../../tests/fixtures/browse_anime/ext_untrusted.json")),
    ("languages.json", include_str!("../../tests/fixtures/browse_anime/languages.json")),
    ("sources_browse.json", include_str!("../../tests/fixtures/browse_anime/sources_browse.json")),
    ("anime_pair_episodes.json", include_str!("../../tests/fixtures/browse_anime/anime_pair_episodes.json")),
    ("anime_pair_tracks.json", include_str!("../../tests/fixtures/browse_anime/anime_pair_tracks.json")),
    ("migrate_flags_defaults.json", include_str!("../../tests/fixtures/browse_anime/migrate_flags_defaults.json")),
    ("season_list.json", include_str!("../../tests/fixtures/browse_anime/season_list.json")),
    ("search_results.json", include_str!("../../tests/fixtures/browse_anime/search_results.json")),
    ("display_names.json", include_str!("../../tests/fixtures/browse_anime/display_names.json")),
];

/// The English base strings of section 24, as `{ key: text }`.
const STRINGS: &str = include_str!("../../tests/fixtures/browse_anime/strings.json");

pub fn fixture(name: &str) -> Value {
    let (_, text) = FIXTURES.iter().find(|(n, _)| *n == name).expect("a fixture of section 5");
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{name} is JSON: {e}"))
}

fn sources_of(value: &Value) -> Vec<ExtSource> {
    value["sources"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|s| ExtSource {
                    id: s["id"].as_i64().expect("id"),
                    name: s["name"].as_str().expect("name").to_owned(),
                    lang: s["lang"].as_str().expect("lang").to_owned(),
                    base_url: s["base_url"].as_str().map(str::to_owned),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn ext_of(value: &Value, state: ExtState) -> Ext {
    Ext {
        name: value["name"].as_str().expect("name").to_owned(),
        pkg: value["pkg"].as_str().expect("pkg").to_owned(),
        version: value["version"].as_str().unwrap_or_default().to_owned(),
        lang: value["lang"].as_str().map(str::to_owned),
        nsfw: value["nsfw"].as_bool().unwrap_or(false),
        sources: sources_of(value),
        state,
        icon_file: None,
    }
}

/// The three extension fixtures as the model rows the list works on.
pub fn extension_fixture() -> Vec<Ext> {
    let installed = fixture("ext_installed.json");
    let available = fixture("ext_available.json");
    let untrusted = fixture("ext_untrusted.json");
    let installed = installed.as_array().expect("array").iter().map(|v| {
        let state = ExtState::Installed { has_update: v["has_update"].as_bool().unwrap_or(false), obsolete: v["obsolete"].as_bool().unwrap_or(false) };
        ext_of(v, state)
    });
    let available = available.as_array().expect("array").iter().map(|v| ext_of(v, ExtState::Available));
    let untrusted = untrusted.as_array().expect("array").iter().map(|v| Ext { lang: None, sources: Vec::new(), ..ext_of(v, ExtState::Untrusted) });
    installed.chain(available).chain(untrusted).collect()
}

fn enabled(languages: &[&str]) -> BTreeSet<String> {
    languages.iter().map(|l| (*l).to_owned()).collect()
}

#[test]
fn every_fixture_of_section_5_is_checked_in_and_is_json() {
    let wanted = [
        "ext_installed.json",
        "ext_available.json",
        "ext_untrusted.json",
        "languages.json",
        "sources_browse.json",
        "anime_pair_episodes.json",
        "anime_pair_tracks.json",
        "migrate_flags_defaults.json",
        "season_list.json",
        "search_results.json",
        "display_names.json",
    ];
    for name in wanted {
        let value = fixture(name);
        assert!(value.is_array() || value.is_object(), "{name}");
    }
    assert_eq!(FIXTURES.len(), wanted.len());
    assert_eq!(fixture("ext_installed.json").as_array().map(Vec::len), Some(4));
    assert_eq!(fixture("ext_available.json").as_array().map(Vec::len), Some(5));
    assert_eq!(fixture("ext_untrusted.json").as_array().map(Vec::len), Some(2));
    assert_eq!(fixture("season_list.json").as_array().map(|l| l.iter().filter(|s| s["favorite"] == true).count()), Some(2));
    assert_eq!(fixture("search_results.json").as_array().map(Vec::len), Some(4));
}

#[test]
fn t1_t9_the_available_list_splits_by_source_and_drops_what_is_installed() {
    let exts = extension_fixture();
    let rows = split_available(&exts, Some(&enabled(&["all", "en"])));
    let pkgs: Vec<&str> = rows.iter().map(|e| e.pkg.as_str()).collect();
    assert!(pkgs.contains(&"eu.example.echo-41") && pkgs.contains(&"eu.example.echo-42"), "T1: <pkg>-<source id>, id as decimal: {pkgs:?}");
    assert!(!pkgs.iter().any(|p| p.starts_with("eu.example.foxtrot")), "German is not enabled");
    assert_eq!(pkgs.iter().filter(|p| p.starts_with("eu.example.alpha")).count(), 1, "T9: the available copy of an installed package is dropped");
    assert!(pkgs.contains(&"eu.example.india-71"), "a store-less listing is a row too");
}

#[test]
fn t3_an_untrusted_extension_ignores_the_nsfw_switch() {
    let rows = split_available(&extension_fixture(), Some(&enabled(&["all", "en", "de"])));
    let shown: Vec<String> = group(&rows, "", false).iter().flat_map(|g| g.rows.iter().map(|&i| rows[i].name.clone())).collect();
    assert!(shown.contains(&"Juliet".to_owned()), "an untrusted NSFW extension shows with show_nsfw off: {shown:?}");
    assert!(!shown.contains(&"Bravo".to_owned()), "an installed NSFW one does not");
    assert!(!shown.contains(&"Foxtrot".to_owned()), "nor an available one");
}

#[test]
fn t10_equal_names_keep_the_order_they_came_in() {
    let rows = split_available(&extension_fixture(), Some(&enabled(&["en"])));
    let groups = group(&rows, "", true);
    let en = groups.iter().find(|g| g.key == GroupKey::Language("en".into())).expect("the en group");
    let names: Vec<&str> = en.rows.iter().map(|&i| rows[i].name.as_str()).collect();
    assert_eq!(names, ["Echo EN", "Hotel", "Hotel", "India"], "ordered by name, ignoring case");
    let hotels: Vec<&str> = en.rows.iter().map(|&i| &rows[i]).filter(|e| e.name == "Hotel").map(|e| e.pkg.as_str()).collect();
    assert_eq!(hotels, ["eu.example.hotel-61", "eu.example.hotel-62"], "two sources with one name stay in snapshot order");
}

#[test]
fn t13_no_enabled_language_leaves_no_available_rows_and_the_empty_text() {
    let rows = split_available(&extension_fixture(), Some(&BTreeSet::new()));
    assert!(rows.iter().all(|e| e.state != ExtState::Available), "no language, no Available row");
    assert!(group(&rows, "", true).iter().all(|g| !matches!(g.key, GroupKey::Language(_))));
    assert_eq!(super::extensions::empty_state("", false), Some(super::extensions::ExtEmpty::Nothing));
    assert_eq!(tr(MsgKey::empty_screen), "Well, this is awkward");
}

fn named(name: &str, source_id: i64) -> Ext {
    Ext {
        name: name.into(),
        pkg: "p".into(),
        version: String::new(),
        lang: Some("en".into()),
        nsfw: false,
        sources: vec![ExtSource { id: source_id, name: "S".into(), lang: "en".into(), base_url: None }],
        state: ExtState::Available,
        icon_file: None,
    }
}

#[test]
fn t5_a_number_matches_that_source_id_exactly_and_leading_zeros_do_not_matter() {
    let e = named("Plain", 123);
    assert!(matches_query("123", &e));
    assert!(!matches_query("1234", &e), "1234 is another id");
    assert!(!matches_query("12", &e), "and 12 is not a prefix match");
    assert!(matches_query("0123", &e), "0123 parses as 123");
}

#[test]
fn t6_the_query_is_lowercased_with_unicode_rules_and_not_case_folded_further() {
    assert!(matches_query("ß", &named("Straße", 1)), "ß finds itself");
    assert!(!matches_query("STRASSE", &named("Straße", 1)), "ß is not expanded to ss: lowercasing, not full case folding");
    assert!(matches_query("İ", &named("İstanbul", 1)), "İ lowercases to i with a dot above and finds itself");
    assert!(matches_query("i", &named("İstanbul", 1)), "the lowercase of İ starts with a plain i");
    assert!(matches_query("ÉCOLE", &named("école", 1)));
}

#[test]
fn t7_only_names_ids_and_urls_are_searched_never_the_store() {
    let available = fixture("ext_available.json");
    let echo = &available.as_array().expect("array")[0];
    assert_eq!(echo["store"], "Main store", "the fixture lists a store");
    let ext = ext_of(echo, ExtState::Available);
    assert!(!matches_query("Main store", &ext), "the store name alone does not match");
    assert!(!matches_query("store", &ext));
    assert!(matches_query("echo", &ext));
}

#[test]
fn t25_cookies_are_removed_once_per_url_and_an_error_counts_as_zero() {
    let source = |id, url: &str| DetailSource { id, name: "S".into(), lang: "en".into(), label_as_name: true, configurable: false, base_url: Some(url.into()) };
    let sources = [source(1, "https://ok.test"), source(2, "https://ok.test"), source(3, "https://broken.test")];
    let calls = std::cell::RefCell::new(Vec::new());
    let (urls, cleared) = clear_cookies(&sources, |url| {
        calls.borrow_mut().push(url.to_owned());
        // a failing removal reports zero, like the catch of the Kotlin code
        if url.contains("broken") { 0 } else { 4 }
    });
    assert_eq!(*calls.borrow(), ["https://ok.test", "https://broken.test"], "once per url");
    assert_eq!(urls.len(), 2);
    assert_eq!(cleared, 4);
}

#[test]
fn t27_an_extension_without_sources_has_no_rows_and_no_panic() {
    assert!(detail_rows(&[], &BTreeSet::new()).is_empty());
    let torrent = fixture("ext_installed.json");
    let delta = ext_of(&torrent.as_array().expect("array")[3], ExtState::Installed { has_update: false, obsolete: false });
    assert!(delta.sources.is_empty(), "the torrent extension has no sources");
}

#[test]
fn t29_a_failed_decode_falls_back_to_the_default_glyph_and_a_failed_fetch_to_the_error_one() {
    use super::icons::{decode_icon, fetch_icon, icon_ref, IconRef};
    assert_eq!(decode_icon(b"garbage"), None, "a decode error shows the default glyph");
    assert_eq!(super::icons::fallback_glyph(false), ("application-x-addon-symbolic", false));
    let dir = tempfile::tempdir().expect("dir");
    assert_eq!(fetch_icon(Some(dir.path()), "https://x.test/missing.png", |_| None), None, "a 404 has no picture");
    assert!(matches!(icon_ref("https://x.test/missing.png"), IconRef::Url(_)));
    assert_eq!(super::icons::fallback_glyph(true), ("dialog-error-symbolic", true), "and shows the error placeholder");
}

#[test]
fn t38_source_ids_are_decimal_strings_and_nothing_parses_them() {
    use kuro_domain::source::model::Source;
    let sources = [Source::new(12, "Twelve", "en")];
    let hidden: BTreeSet<String> = ["x".to_owned()].into();
    let list = super::sources::source_list(&super::sources::ListInput {
        sources: &sources,
        enabled_languages: &enabled(&["en"]),
        disabled: &hidden,
        pinned: &BTreeSet::new(),
        last_used: -1,
    });
    assert_eq!(list.len(), 2, "a stored id that is not a number is skipped, the source still lists: {list:?}");
    let hidden: BTreeSet<String> = ["12".to_owned()].into();
    let list = super::sources::source_list(&super::sources::ListInput {
        sources: &sources,
        enabled_languages: &enabled(&["en"]),
        disabled: &hidden,
        pinned: &BTreeSet::new(),
        last_used: -1,
    });
    assert!(list.is_empty(), "\"12\" is the id of the source");
}

#[test]
fn t40_the_search_starts_with_every_source_and_keeps_the_origin_first() {
    use super::pages::{SearchChips, SearchRow, SearchState, SourceFilter, visible_rows};
    use kuro_domain::source::model::Source;
    assert_eq!(SearchChips::default().filter, SourceFilter::All, "the pinned-only filter is cut");
    let row = |id: i64, origin: bool| SearchRow { source: Source::new(id, "S", "en"), state: SearchState::Loading, origin };
    let rows = vec![row(1, true), row(2, false)];
    let shown = visible_rows(&rows, SearchChips::default());
    assert_eq!(shown.iter().map(|r| r.source.id).collect::<Vec<_>>(), [1, 2], "every source shows, the origin first");
}

#[test]
fn t41_the_search_subtitle_is_the_localized_language_name() {
    // the names are the ones Locale.getDisplayName(locale) gives in the language itself, and `all` reads as "All"
    let names = fixture("display_names.json");
    for (tag, expected) in names.as_object().expect("object") {
        assert_eq!(subtitle_display_name(tag), *expected.as_str().expect("text"), "{tag}");
    }
    assert_eq!(localized_display_name("all"), "Multi", "the source lists keep Multi");
}

/// Turns the Android placeholders of the table into the Fluent arguments the message takes.
fn english_of(key: &MsgKey) -> String {
    if key.args().is_empty() {
        tr(*key)
    } else {
        trf(*key, &[("arg1", Arg::Str("X"))])
    }
}

#[test]
fn every_string_of_section_24_matches_the_english_base_text() {
    let table: serde_json::Map<String, Value> = serde_json::from_str(STRINGS).expect("strings.json");
    assert!(table.len() > 80, "section 24 lists more than eighty keys");
    let mut drift = Vec::new();
    for (id, expected) in &table {
        let expected = expected.as_str().expect("text");
        let Some(key) = MsgKey::ALL.iter().find(|k| k.id() == id) else {
            drift.push(format!("{id}: no such key"));
            continue;
        };
        // the plan writes `%s` and `%1$s` where Fluent takes `arg1`, and runs a paragraph break into a space
        let squash = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        let want = squash(&expected.replace("%1$s", "X").replace("%s", "X"));
        let got = squash(&english_of(key));
        if got != want {
            drift.push(format!("{id}: plan {want:?}, app {got:?}"));
        }
    }
    assert!(drift.is_empty(), "the English base text drifted from section 24:\n{}", drift.join("\n"));
}

// ---------- integration tests I1 to I4 (need a display, run from `smoke`) ----------

/// I1: the Extensions page over the fixtures: the section headers come in order, and Update all is on the updates
/// header only.
fn i1_extensions_page_over_the_fixtures() {
    use std::rc::Rc;
    use std::sync::Arc;

    use gtk::prelude::*;

    use super::extensions_page::{ExtNav, ExtensionBackend, ExtensionsPage, JobFuture, RefreshFuture, Reporter};

    struct Idle;
    impl ExtensionBackend for Idle {
        fn install(&self, _pkg: String, _report: Reporter) -> JobFuture {
            Box::pin(async {})
        }
        fn uninstall(&self, _pkg: String) -> JobFuture {
            Box::pin(async {})
        }
        fn refresh(&self) -> RefreshFuture {
            Box::pin(async { Ok(()) })
        }
    }
    fn labels(widget: &gtk::Widget, out: &mut Vec<String>) {
        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
            out.push(label.label().to_string());
        }
        // a button's text is the label inside it
        let mut child = widget.first_child();
        while let Some(w) = child {
            labels(&w, out);
            child = w.next_sibling();
        }
    }
    let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("runtime");
    let _entered = rt.enter();
    let nav = ExtNav {
        open_details: Rc::new(|_| {}),
        trust: Rc::new(|_| {}),
        web_view: Rc::new(|_| {}),
        open_repos: Rc::new(|| {}),
        repos_label: MsgKey::extensionStores,
        show_nsfw: Rc::new(|| true),
    };
    let page = ExtensionsPage::new(Arc::new(Idle), nav);
    page.set_enabled_languages(Some(enabled(&["all", "en"])));
    page.set_exts(extension_fixture());
    let mut text = Vec::new();
    labels(page.widget().upcast_ref(), &mut text);
    let position = |wanted: String| text.iter().position(|t| *t == wanted).unwrap_or_else(|| panic!("{wanted} in {text:?}"));
    let (updates, installed, all_group) = (position(tr(MsgKey::ext_updates_pending)), position(tr(MsgKey::ext_installed)), position(localized_display_name("all")));
    assert!(updates < installed && installed < all_group, "Updates pending, Installed, then the languages: {text:?}");
    assert_eq!(text.iter().filter(|t| **t == tr(MsgKey::ext_update_all)).count(), 1, "Update all is on the updates header only");
    let mut without = extension_fixture();
    for ext in &mut without {
        if let ExtState::Installed { has_update, .. } = &mut ext.state {
            *has_update = false;
        }
    }
    page.set_exts(without);
    let mut text = Vec::new();
    labels(page.widget().upcast_ref(), &mut text);
    assert!(!text.contains(&tr(MsgKey::ext_update_all)), "no updates, no Update all button");
}

/// I2: the error-only state of a source page offers its actions in order.
fn i2_error_state_buttons_in_order() {
    use gtk::prelude::*;

    use super::primitives::{EmptyAction, error_actions};
    let actions = error_actions(false);
    let page = super::view::empty_state("failed", &actions, |_: EmptyAction| {});
    let mut tips = Vec::new();
    let mut stack = vec![page.upcast::<gtk::Widget>()];
    while let Some(w) = stack.pop() {
        if let Some(button) = w.downcast_ref::<gtk::Button>() {
            tips.push(button.tooltip_text().map(|t| t.to_string()).unwrap_or_default());
        }
        let mut children = Vec::new();
        let mut child = w.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            children.push(c);
        }
        stack.extend(children.into_iter().rev());
    }
    let expected: Vec<String> = actions.iter().map(|a| a.label()).collect();
    assert_eq!(tips, expected, "Retry, Open in WebView, then Help when there is a help page");
    assert_eq!(expected[0], tr(MsgKey::action_retry));
    assert_eq!(expected[1], tr(MsgKey::action_open_in_web_view));
}

/// I3: the migrate dialog has no widget in this lane: the migration lane owns it, so the check is skipped, loudly.
fn i3_migrate_dialog_skipped() {
    eprintln!("I3 skipped: the migrate dialog (D1) is built by the migration lane; the browse lane has no such widget to assert on");
}

/// I4: global search with one source loading and one answered: the progress bar is at one half.
fn i4_global_search_progress_is_a_half() {
    use std::rc::Rc;

    use kuro_domain::source::model::Source;
    use tokio::sync::watch;

    use super::listing::ListItem;
    use super::pages::{GlobalSearchNav, GlobalSearchSnapshot, SearchChips, SearchRow, SearchState, SourceFilter, global_search};
    let nav = GlobalSearchNav {
        search: Rc::new(|_| {}),
        open_source: Rc::new(|_, _| {}),
        open_entry: Rc::new(|_, _| {}),
        preview: Rc::new(|_, _| {}),
        load_cover: Rc::new(|_, _| {}),
        chips: SearchChips { filter: SourceFilter::All, only_has_results: false },
        on_chips: Rc::new(|_| {}),
    };
    let page = global_search(nav, watch::channel(true).1);
    let row = |id, state| SearchRow { source: Source::new(id, "S", "en"), state, origin: false };
    page.update(&GlobalSearchSnapshot {
        query: "q".into(),
        answered: 1,
        total: 2,
        rows: vec![row(1, SearchState::Results(vec![ListItem::default()])), row(2, SearchState::Loading)],
    });
    assert_eq!(page.progress(), Some(0.5), "one of two sources answered");
}

pub(crate) fn smoke() {
    i1_extensions_page_over_the_fixtures();
    i2_error_state_buttons_in_order();
    i3_migrate_dialog_skipped();
    i4_global_search_progress_is_a_half();
}
