//! Source lists of the app layer (`GetEnabledMangaSources`, `GetEnabledAnimeSources`, `ToggleMangaSource`,
//! `ToggleMangaSourcePin`, `ToggleLanguage`; T-10a-018, T-10a-020). The two kinds share one algorithm, parameterised by
//! the preference handles of each kind.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::Mutex;

use kuro_prefs::pref::Pref;
use tokio::sync::watch;

use super::rx::{combine2, combine3, pref_stream};
use crate::source::toggles::{flip_disabled, set_disabled, toggle_data_saver_excluded, toggle_language, toggle_pin};
use crate::source::model::{Id, Source};
use crate::source::pin::{Pin, Pins};
use kuro_prefs::source::SourcePrefs;

/// The id of the local source, which is listed whatever the language filter says (`LocalMangaSource.ID`).
pub const LOCAL_SOURCE_ID: Id = 0;

/// Manga or anime: which preference keys a source list reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Manga,
    Anime,
}

/// The preference handles of one kind of source list.
#[derive(Clone)]
pub struct KindPrefs {
    /// Decimal ids of the pinned sources.
    pub pinned: Pref<BTreeSet<String>>,
    /// Decimal ids of the hidden sources.
    pub disabled: Pref<BTreeSet<String>>,
    /// The id of the source used last, or `-1`.
    pub last_used: Pref<i64>,
}

impl KindPrefs {
    pub fn of(prefs: &SourcePrefs, kind: Kind) -> Self {
        match kind {
            Kind::Manga => Self {
                pinned: prefs.pinned_manga_sources.clone(),
                disabled: prefs.disabled_manga_sources.clone(),
                last_used: prefs.last_used_manga_source.clone(),
            },
            Kind::Anime => Self {
                pinned: prefs.pinned_anime_sources.clone(),
                disabled: prefs.disabled_anime_sources.clone(),
                last_used: prefs.last_used_anime_source.clone(),
            },
        }
    }
}

/// The list the screens show (`GetEnabledMangaSources` and its anime twin, without the stream): sources in an enabled
/// language (the local source always), not hidden, sorted by name without regard to case; pinned sources carry
/// `Pins::PINNED`. The source used last is listed a second time, right after its first row, with `is_used_last` set and
/// the pin without `Actual` (EC-31; `without`, not Kotlin's XOR, so an unpinned duplicate is `0b00`, accepted difference 3).
/// A last-used id that is hidden or unknown adds nothing (EC-32). `excluded` is the data-saver exclusion set for manga;
/// anime passes `None`, so its rows never carry the flag.
pub fn compute_enabled_sources(
    sources: &[Source],
    languages: &BTreeSet<String>,
    disabled: &BTreeSet<String>,
    pinned: &BTreeSet<String>,
    last_used: i64,
    excluded: Option<&BTreeSet<String>>,
) -> Vec<Source> {
    let mut listed: Vec<&Source> = sources
        .iter()
        .filter(|s| languages.contains(&s.lang) || s.id == LOCAL_SOURCE_ID)
        .filter(|s| !disabled.contains(&s.id.to_string()))
        .collect();
    // A stable sort keeps the repository order of sources that share a name (EC-33).
    listed.sort_by(|a, b| cmp_ignore_case_java(&a.name, &b.name));

    let mut out = Vec::with_capacity(listed.len());
    for source in listed {
        let pin = if pinned.contains(&source.id.to_string()) { Pins::PINNED } else { Pins::UNPINNED };
        let data_saver_excluded = excluded.is_some_and(|set| set.contains(&source.id.to_string()));
        let row = Source { pin, data_saver_excluded, ..source.clone() };
        out.push(row.clone());
        if source.id == last_used {
            out.push(Source { is_used_last: true, pin: pin.without(Pin::Actual), ..row });
        }
    }
    out
}

/// The enabled list as a stream: the current list first, then a new list after each change of the languages, the
/// hidden or pinned ids, the last-used id or the source list. A list equal to the last one is not published
/// (`distinctUntilChanged`).
pub fn subscribe_enabled(
    prefs: &SourcePrefs,
    kind: Kind,
    sources: watch::Receiver<Vec<Source>>,
) -> watch::Receiver<Vec<Source>> {
    let kind_prefs = KindPrefs::of(prefs, kind);
    // The data-saver set is watched for both kinds, so every input stays open; `enabled_from` reads it for manga only.
    let inputs = combine2(
        combine3(
            pref_stream(&kind_prefs.pinned),
            pref_stream(&prefs.enabled_languages),
            pref_stream(&kind_prefs.disabled),
        ),
        combine3(
            pref_stream(&kind_prefs.last_used),
            pref_stream(&prefs.data_saver_excluded_sources),
            sources,
        ),
    );
    let (tx, rx) = watch::channel(enabled_from(kind, &inputs.borrow()));
    let mut inputs = inputs;
    tokio::spawn(async move {
        while inputs.changed().await.is_ok() {
            let list = enabled_from(kind, &inputs.borrow_and_update());
            if tx.is_closed() {
                break;
            }
            tx.send_if_modified(|current| {
                if *current == list {
                    false
                } else {
                    *current = list;
                    true
                }
            });
        }
    });
    rx
}

/// The enabled list from the combined inputs of [`subscribe_enabled`]: the pinned, language and hidden sets, then the
/// last-used id, the data-saver exclusion set and the source list. Only a manga list reads the exclusion set, so no anime
/// row carries the flag.
type EnabledInputs = ((BTreeSet<String>, BTreeSet<String>, BTreeSet<String>), (i64, BTreeSet<String>, Vec<Source>));

fn enabled_from(kind: Kind, ((pinned, languages, disabled), (last_used, excluded, sources)): &EnabledInputs) -> Vec<Source> {
    let excluded = (kind == Kind::Manga).then_some(excluded);
    compute_enabled_sources(sources, languages, disabled, pinned, *last_used, excluded)
}

/// The Java `String.CASE_INSENSITIVE_ORDER` (JDK 16 and later, which compare code points): each character is compared
/// after upper-casing and then lower-casing, and a string that is a prefix of the other sorts first. The Kotlin list
/// sorts with this order (fixture `tests/fixtures/java_ci_order.json`).
pub fn cmp_ignore_case_java(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars(), b.chars());
    loop {
        match (x.next(), y.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c1), Some(c2)) if c1 != c2 => {
                let (u1, u2) = (java_upper(c1), java_upper(c2));
                if u1 != u2 {
                    let (l1, l2) = (java_lower(u1), java_lower(u2));
                    if l1 != l2 {
                        return l1.cmp(&l2);
                    }
                }
            }
            (Some(_), Some(_)) => {}
        }
    }
}

/// `Character.toUpperCase`: the single-character upper case, or the character itself when the Unicode mapping has
/// several characters.
fn java_upper(c: char) -> char {
    let mut mapped = c.to_uppercase();
    match (mapped.next(), mapped.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

/// `Character.toLowerCase`. U+0130 maps to `i` in Java's simple mapping, not to the two-character Unicode one.
fn java_lower(c: char) -> char {
    if c == '\u{130}' {
        return 'i';
    }
    let mut mapped = c.to_lowercase();
    match (mapped.next(), mapped.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

/// The toggles of the source lists, one persisted write each. They run one at a time under this lock, so two toggles
/// cannot interleave (E-C3). The set logic is in [`super::toggles`]; this type only reads the preference and writes it back.
#[derive(Default)]
pub struct SourceToggles {
    lock: Mutex<()>,
}

impl SourceToggles {
    /// Flips source `id` between shown and hidden. Returns `true` when hidden after the flip.
    pub fn flip_disabled(&self, hidden: &Pref<BTreeSet<String>>, id: Id) -> bool {
        self.edit(hidden, |set| flip_disabled(set, id))
    }

    /// Shows or hides every source in `ids` in one write. Returns `disable`.
    pub fn set_disabled(&self, hidden: &Pref<BTreeSet<String>>, ids: &[Id], disable: bool) -> bool {
        self.edit(hidden, |set| set_disabled(set, ids, disable))
    }

    /// Pins or unpins source `id`. Returns `true` when pinned after the call.
    pub fn toggle_pin(&self, pinned: &Pref<BTreeSet<String>>, id: Id) -> bool {
        self.edit(pinned, |set| toggle_pin(set, id))
    }

    /// Adds or removes a language. Returns `true` when enabled after the call.
    pub fn toggle_language(&self, languages: &Pref<BTreeSet<String>>, language: &str) -> bool {
        self.edit(languages, |set| toggle_language(set, language))
    }

    /// Puts source `id` in or out of the data-saver exclusion set. Returns `true` when excluded after the call.
    pub fn toggle_data_saver_excluded(&self, excluded: &Pref<BTreeSet<String>>, id: Id) -> bool {
        self.edit(excluded, |set| toggle_data_saver_excluded(set, id))
    }

    /// One read-modify-write of `pref` under the lock.
    fn edit(&self, pref: &Pref<BTreeSet<String>>, change: impl FnOnce(&mut BTreeSet<String>) -> bool) -> bool {
        let _guard = self.lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut set = pref.get();
        let result = change(&mut set);
        pref.set(set);
        result
    }
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::app::rx::TEST_WAIT;

    fn test_prefs() -> SourcePrefs {
        SourcePrefs::new(PrefStore::in_memory())
    }

    fn src(id: Id, name: &str, lang: &str) -> Source {
        Source::new(id, name, lang)
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn keys(list: &[Source]) -> Vec<String> {
        list.iter().map(Source::key).collect()
    }

    #[test]
    fn empty_languages_leave_only_the_local_source() {
        let sources = [src(0, "Local", ""), src(1, "Alpha", "en"), src(2, "Beta", "ja")];
        let list = compute_enabled_sources(&sources, &set(&[]), &set(&[]), &set(&[]), -1, None);
        assert_eq!(keys(&list), ["0"], "EC-30");
    }

    #[test]
    fn pinned_and_last_used_source_gives_two_rows_with_the_second_flagged() {
        let sources = [src(1, "Alpha", "en"), src(2, "Beta", "en")];
        let list = compute_enabled_sources(&sources, &set(&["en"]), &set(&[]), &set(&["1"]), 1, None);
        assert_eq!(keys(&list), ["1", "1-lastused", "2"], "EC-31: the second row has the used-last key");
        assert_eq!(list[0].pin, Pins::PINNED);
        assert_eq!(list[1].pin.bits(), 0b01, "pin minus Actual");
        assert!(list[1].is_used_last && !list[0].is_used_last);
    }

    #[test]
    fn a_last_used_id_that_is_hidden_or_unknown_adds_no_row() {
        let sources = [src(1, "Alpha", "en")];
        let hidden = compute_enabled_sources(&sources, &set(&["en"]), &set(&["1"]), &set(&[]), 1, None);
        assert!(hidden.is_empty(), "EC-32: hidden, so no duplicate");
        let unknown = compute_enabled_sources(&sources, &set(&["en"]), &set(&[]), &set(&[]), 99, None);
        assert_eq!(keys(&unknown), ["1"]);
    }

    #[test]
    fn names_sort_without_regard_to_case_and_equal_names_keep_repository_order() {
        let sources = [src(7, "beta", "en"), src(3, "Alpha", "en"), src(9, "Beta", "ja"), src(4, "ALPHA", "en")];
        let list = compute_enabled_sources(&sources, &set(&["en", "ja"]), &set(&[]), &set(&[]), -1, None);
        assert_eq!(keys(&list), ["3", "4", "7", "9"], "EC-33: stable among equal names");
    }

    #[test]
    fn the_java_order_matches_the_jvm_fixture_on_every_pair() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/java_ci_order.json")).expect("fixture");
        let items: Vec<String> = fixture["items"].as_array().expect("items").iter().map(|v| v.as_str().unwrap().to_owned()).collect();
        for pair in fixture["pairs"].as_array().expect("pairs") {
            let (a, b, sign) = (pair[0].as_str().unwrap(), pair[1].as_str().unwrap(), pair[2].as_i64().unwrap());
            let got = match cmp_ignore_case_java(a, b) {
                Ordering::Less => -1,
                Ordering::Equal => 0,
                Ordering::Greater => 1,
            };
            assert_eq!(got, sign, "compare({a:?}, {b:?})");
        }
        let mut sorted = items.clone();
        sorted.sort_by(|a, b| cmp_ignore_case_java(a, b));
        let expected: Vec<String> = fixture["sorted"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn toggles_flip_show_hide_and_pin_and_languages() {
        let prefs = test_prefs();
        let toggles = SourceToggles::default();
        assert!(toggles.flip_disabled(&prefs.disabled_manga_sources, 5), "flip hides and reports hidden");
        assert!(prefs.disabled_manga_sources.get().contains("5"));
        assert!(!toggles.flip_disabled(&prefs.disabled_manga_sources, 5), "flip shows and reports shown");
        assert!(prefs.disabled_manga_sources.get().is_empty());

        assert!(toggles.set_disabled(&prefs.disabled_manga_sources, &[5, 6], true));
        assert_eq!(prefs.disabled_manga_sources.get(), set(&["5", "6"]));
        assert!(!toggles.set_disabled(&prefs.disabled_manga_sources, &[5, 6], false));
        assert!(prefs.disabled_manga_sources.get().is_empty());

        assert!(toggles.toggle_pin(&prefs.pinned_manga_sources, 8));
        assert!(prefs.pinned_manga_sources.get().contains("8"));
        assert!(!toggles.toggle_pin(&prefs.pinned_manga_sources, 8));
        assert!(prefs.pinned_manga_sources.get().is_empty());

        assert!(toggles.toggle_language(&prefs.enabled_languages, "fr"));
        assert!(prefs.enabled_languages.get().contains("fr"));
        assert!(!toggles.toggle_language(&prefs.enabled_languages, "fr"));
        assert!(!prefs.enabled_languages.get().contains("fr"));

        assert!(toggles.toggle_data_saver_excluded(&prefs.data_saver_excluded_sources, 9));
        assert!(!toggles.toggle_data_saver_excluded(&prefs.data_saver_excluded_sources, 9));
    }

    #[test]
    fn a_toggle_with_one_write_leaves_the_other_sets_as_they_were() {
        let prefs = test_prefs();
        let toggles = SourceToggles::default();
        toggles.toggle_pin(&prefs.pinned_manga_sources, 3);
        toggles.toggle_language(&prefs.enabled_languages, "ja");
        toggles.flip_disabled(&prefs.disabled_manga_sources, 4);
        assert_eq!(prefs.pinned_manga_sources.get(), set(&["3"]), "a language or hide does not touch pins");
        assert_eq!(prefs.disabled_manga_sources.get(), set(&["4"]));
        assert!(prefs.enabled_languages.get().contains("ja"));
    }

    /// The section 2 input of `p1-edge-cases-and-tests.md`, as rows, with the settings the vector names.
    fn section2() -> (Vec<Source>, BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
        let rows = crate::source::fakes::enabled_rows();
        (rows, set(&["en", "ja"]), set(&["4"]), set(&["3"]))
    }

    #[test]
    fn the_main_vector_of_section_two_gives_the_five_rows_in_order() {
        let (rows, languages, disabled, pinned) = section2();
        let list = compute_enabled_sources(&rows, &languages, &disabled, &pinned, 2, Some(&set(&[])));
        assert_eq!(keys(&list), ["2", "2-lastused", "3", "0", "1"]);
        assert_eq!(list[1].pin.bits(), 0b00, "the Rust duplicate of an unpinned source is 0b00");
        assert_eq!(list[2].pin, Pins::PINNED);
        assert!(list[1].is_used_last && !list[1].pin.contains(Pin::Actual));
    }

    #[test]
    fn e_n1_no_languages_leaves_only_the_local_row() {
        let (rows, _, disabled, pinned) = section2();
        let list = compute_enabled_sources(&rows, &set(&[]), &disabled, &pinned, 2, Some(&set(&[])));
        assert_eq!(keys(&list), ["0"]);
    }

    #[test]
    fn e_n2_hidden_pinned_and_last_used_leave_rows_four_and_five_only() {
        // The vector's disabled set {2, 3} is read with source 4 still hidden (the main setting), so the output is the
        // local row and Zeta: rows 4 and 5 of the main vector.
        let (rows, languages, _, _) = section2();
        let list = compute_enabled_sources(&rows, &languages, &set(&["2", "3", "4"]), &set(&[]), 2, Some(&set(&[])));
        assert_eq!(keys(&list), ["0", "1"]);
    }

    #[test]
    fn e_n3_an_unknown_last_used_id_adds_no_duplicate() {
        let (rows, languages, disabled, pinned) = section2();
        let list = compute_enabled_sources(&rows, &languages, &disabled, &pinned, 99, Some(&set(&[])));
        assert!(list.iter().all(|s| !s.is_used_last));
    }

    #[test]
    fn e_n4_a_pinned_source_outside_the_languages_is_absent_but_stays_pinned() {
        let (rows, _, _, pinned) = section2();
        let languages = set(&["fr"]);
        let list = compute_enabled_sources(&rows, &languages, &set(&["4"]), &pinned, -1, Some(&set(&[])));
        assert_eq!(keys(&list), ["0", "5"], "source 3 is absent");
        assert!(pinned.contains("3"), "the set is storage and is not changed by the list");
    }

    #[test]
    fn e_n5_a_last_used_source_in_a_third_language_appears_twice() {
        let (rows, _, disabled, pinned) = section2();
        let languages = set(&["en", "ja", "fr"]);
        let list = compute_enabled_sources(&rows, &languages, &disabled, &pinned, 5, Some(&set(&[])));
        let omega = list.iter().filter(|s| s.id == 5).count();
        assert_eq!(omega, 2);
    }

    /// The section 2 vectors through `enabled_from`, the path both kinds take. The data-saver set is empty here, so the
    /// manga and anime lists agree row for row (T-17a-042).
    #[test]
    fn section_two_vectors_hold_for_manga_and_anime() {
        let (rows, languages, disabled, pinned) = section2();
        let run = |kind: Kind, languages: &BTreeSet<String>, hidden: &BTreeSet<String>, last_used: i64| {
            let inputs = ((pinned.clone(), languages.clone(), hidden.clone()), (last_used, set(&[]), rows.clone()));
            keys(&enabled_from(kind, &inputs))
        };
        for kind in [Kind::Manga, Kind::Anime] {
            let main = run(kind, &languages, &disabled, 2);
            assert_eq!(main, ["2", "2-lastused", "3", "0", "1"], "main vector, {kind:?}");
            assert_eq!(run(kind, &set(&[]), &disabled, 2), ["0"], "E-N1, {kind:?}");
            assert_eq!(run(kind, &languages, &set(&["2", "3", "4"]), 2), ["0", "1"], "E-N2, {kind:?}");
            assert_eq!(run(kind, &languages, &disabled, 99), ["2", "3", "0", "1"], "E-N3, {kind:?}");
            assert_eq!(run(kind, &set(&["fr"]), &set(&["4"]), -1), ["0", "5"], "E-N4, {kind:?}");
            let e_n5 = run(kind, &set(&["en", "ja", "fr"]), &disabled, 5);
            assert_eq!(e_n5.iter().filter(|key| key.starts_with('5')).count(), 2, "E-N5, {kind:?}");
        }
    }

    #[test]
    fn a_pinned_last_used_duplicate_keeps_the_pinned_bit_so_its_menu_reads_unpin() {
        let sources = [src(1, "Alpha", "en")];
        let list = compute_enabled_sources(&sources, &set(&["en"]), &set(&[]), &set(&["1"]), 1, Some(&set(&[])));
        assert_eq!(list.len(), 2);
        assert!(list[1].is_used_last);
        assert!(list[1].pin.contains(Pin::Pinned), "the duplicate reads Unpin, as its source does");
        assert!(!list[1].pin.contains(Pin::Actual));
    }

    #[test]
    fn data_saver_exclusion_marks_manga_rows_and_never_anime_rows() {
        let sources = [src(1, "Alpha", "en"), src(2, "Beta", "en")];
        let excluded = set(&["2"]);
        let manga = compute_enabled_sources(&sources, &set(&["en"]), &set(&[]), &set(&[]), -1, Some(&excluded));
        assert!(!manga[0].data_saver_excluded && manga[1].data_saver_excluded);
        let anime = compute_enabled_sources(&sources, &set(&["en"]), &set(&[]), &set(&[]), -1, None);
        assert!(anime.iter().all(|s| !s.data_saver_excluded), "anime rows never carry the flag");
    }

    #[test]
    fn unparsable_stored_ids_are_ignored_by_the_readers() {
        let sources = [src(1, "Alpha", "en")];
        let list = compute_enabled_sources(&sources, &set(&["en"]), &set(&["x", "1.5"]), &set(&["y"]), -1, None);
        assert_eq!(keys(&list), ["1"]);
        assert_eq!(list[0].pin, Pins::UNPINNED);
    }

    #[tokio::test]
    async fn the_stream_publishes_the_current_list_then_each_real_change() {
        let prefs = test_prefs();
        let (tx, rx) = watch::channel(vec![src(1, "Alpha", "en"), src(2, "Beta", "en")]);
        let mut stream = subscribe_enabled(&prefs, Kind::Manga, rx);
        assert_eq!(keys(&stream.borrow_and_update()), ["1", "2"]);

        let toggles = SourceToggles::default();
        toggles.flip_disabled(&prefs.disabled_manga_sources, 1);
        tokio::time::timeout(TEST_WAIT, stream.changed()).await.expect("waited").expect("open");
        assert_eq!(keys(&stream.borrow_and_update()), ["2"]);

        tx.send(vec![src(2, "Beta", "en")]).expect("same visible list");
        tx.send(vec![src(2, "Beta", "en"), src(3, "Gamma", "en")]).expect("source list");
        tokio::time::timeout(TEST_WAIT, stream.changed()).await.expect("waited").expect("open");
        assert_eq!(keys(&stream.borrow_and_update()), ["2", "3"]);
    }

    #[tokio::test]
    async fn a_change_that_leaves_the_list_equal_is_not_published() {
        let prefs = test_prefs();
        let (_tx, rx) = watch::channel(vec![src(1, "Alpha", "en")]);
        let mut stream = subscribe_enabled(&prefs, Kind::Manga, rx);
        assert_eq!(keys(&stream.borrow_and_update()), ["1"]);
        // A language nobody has: the list is the same, so no publication follows (distinctUntilChanged).
        let toggles = SourceToggles::default();
        toggles.toggle_language(&prefs.enabled_languages, "fr");
        let waited = tokio::time::timeout(std::time::Duration::from_millis(200), stream.changed()).await;
        assert!(waited.is_err(), "no publication for an equal list");
    }
}
