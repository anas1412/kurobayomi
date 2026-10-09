//! Browse buckets (T-19a-027, `p1-rust-design.md` section 7.6). A pure function over the state: no GTK, no disk.

use std::collections::{BTreeMap, BTreeSet};

use crate::anime::manager::ExtensionState;
use crate::anime::model::{AvailableRow, Buckets, Installed, Kind, Untrusted};

/// The four Browse lists. `enabled_langs` is the set of source languages the user turned on. `show_nsfw` is the
/// `show_nsfw_source` setting, read by the caller on every rebuild.
///
/// Installed packages with an update go to `updates`; the rest go to `installed`, obsolete packages first. Untrusted
/// packages are not filtered by NSFW (kept from Aniyomi). Available packages that are installed or untrusted are left
/// out. Each listed package gives one row per source in an enabled language, keyed `<pkg>-<sourceId>`. A package with
/// no sources gives one row with no source, keyed by the package name (a deliberate change from Aniyomi).
///
/// A listed package that is not `Kind::Wasm` (`ApkOnly` or `Unsigned`) cannot run on Linux. It is hidden unless
/// `show_not_runnable` is set, and the hidden count is returned in `not_runnable_hidden` (T-19a-066).
pub fn buckets(
    state: &ExtensionState,
    enabled_langs: &BTreeSet<String>,
    show_nsfw: bool,
    show_not_runnable: bool,
) -> Buckets {
    let mut installed: Vec<Installed> = state
        .installed()
        .values()
        .filter(|ext| show_nsfw || !ext.nsfw)
        .cloned()
        .collect();
    installed.sort_by_key(|ext| (!ext.is_obsolete, ext.name.to_lowercase()));
    let (updates, installed): (Vec<_>, Vec<_>) =
        installed.into_iter().partition(|ext| ext.has_update);

    let mut untrusted: Vec<Untrusted> = state.untrusted().values().cloned().collect();
    untrusted.sort_by_key(|ext| ext.name.to_lowercase());

    let mut available = Vec::new();
    let mut not_runnable_hidden = 0;
    let mut seen = BTreeSet::new();
    for listed in state.available() {
        if !seen.insert(listed.pkg.as_str()) {
            // The first listing of a package is the one browse and install use (design 7.4).
            continue;
        }
        let hidden = state.installed().contains_key(&listed.pkg)
            || state.untrusted().contains_key(&listed.pkg)
            || (listed.nsfw && !show_nsfw);
        if hidden {
            continue;
        }
        if !matches!(listed.kind, Kind::Wasm) && !show_not_runnable {
            not_runnable_hidden += 1;
            continue;
        }
        if listed.sources.is_empty() {
            available.push(AvailableRow {
                ext: listed.clone(),
                source: None,
                row_key: listed.pkg.clone(),
            });
            continue;
        }
        for source in listed
            .sources
            .iter()
            .filter(|source| enabled_langs.contains(&source.lang))
        {
            available.push(AvailableRow {
                ext: listed.clone(),
                source: Some(source.clone()),
                row_key: format!("{}-{}", listed.pkg, source.id),
            });
        }
    }
    available.sort_by_key(|row| (display_name(row).to_lowercase(), row.row_key.clone()));

    Buckets {
        updates,
        installed,
        available,
        untrusted,
        not_runnable_hidden,
    }
}

/// What a Browse row of a listed package says about running it here (T-19a-066). `ApkOnly` rows are not runnable on
/// Linux and `Unsigned` rows lack a valid signature; neither can be installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowBadges {
    pub not_runnable: bool,
    pub unsigned: bool,
}

impl RowBadges {
    pub fn of(kind: Kind) -> Self {
        Self { not_runnable: kind == Kind::ApkOnly, unsigned: kind == Kind::Unsigned }
    }

    /// Install is off for every row that carries a badge.
    pub fn install_enabled(self) -> bool {
        !self.not_runnable && !self.unsigned
    }
}

/// How many entries of each store (by index URL) are hidden as not runnable while `show_not_runnable` is off. The
/// first listing of a package counts, as in [`buckets`]. A store with none has no key.
pub fn not_runnable_by_store(state: &ExtensionState, show_not_runnable: bool) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    if show_not_runnable {
        return counts;
    }
    let mut seen = BTreeSet::new();
    for listed in state.available() {
        if seen.insert(listed.pkg.as_str()) && !matches!(listed.kind, Kind::Wasm) {
            *counts.entry(listed.store.index_url.clone()).or_insert(0) += 1;
        }
    }
    counts
}

/// The name a Browse row shows: the source name for a split row, the package name otherwise.
fn display_name(row: &AvailableRow) -> &str {
    row.source
        .as_ref()
        .map_or(row.ext.name.as_str(), |source| source.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::abi::AbiVersion;
    use crate::anime::model::{AnimeSourceRef, Available, LoadResult, SignerDigest, StoreRef};

    fn source(id: i64, name: &str, lang: &str) -> AnimeSourceRef {
        AnimeSourceRef {
            id,
            name: name.into(),
            lang: lang.into(),
            base_url: String::new(),
        }
    }

    fn installed(pkg: &str, name: &str, nsfw: bool, has_update: bool) -> Installed {
        Installed {
            name: name.into(),
            pkg: pkg.into(),
            version_name: "1.0.0".into(),
            version_code: 1,
            abi: AbiVersion { major: 1, minor: 0 },
            lang: "en".into(),
            nsfw,
            torrent: false,
            sources: Vec::new(),
            icon: None,
            signer: SignerDigest("ab".repeat(32)),
            has_update,
            is_obsolete: false,
            store: None,
        }
    }

    fn listed(
        pkg: &str,
        name: &str,
        nsfw: bool,
        sources: Vec<AnimeSourceRef>,
        index_url: &str,
    ) -> Available {
        Available {
            name: name.into(),
            raw_name: name.into(),
            pkg: pkg.into(),
            version_name: "1.0.0".into(),
            version_code: 1,
            abi: None,
            aniyomi_lib: None,
            lang: "en".into(),
            nsfw,
            torrent: false,
            sources,
            kind: Kind::Wasm,
            apk_url: String::new(),
            icon_url: String::new(),
            kuroext: None,
            store: StoreRef {
                index_url: index_url.into(),
                name: "S".into(),
            },
        }
    }

    fn langs(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|lang| (*lang).to_owned()).collect()
    }

    fn state(installed_list: Vec<Installed>, available: Vec<Available>) -> ExtensionState {
        let mut state = ExtensionState::new();
        state.apply_loaded(
            installed_list
                .into_iter()
                .map(LoadResult::Success)
                .collect(),
        );
        state.apply_available(available);
        state
    }

    #[test]
    fn tst_19_066_installed_is_sorted_with_obsolete_first_then_by_name() {
        let mut gone = installed("eu.gone", "Zed", false, false);
        gone.is_obsolete = true;
        let state = state(
            vec![
                installed("eu.b", "Beta", false, false),
                installed("eu.a", "alpha", false, false),
                gone,
            ],
            vec![
                listed("eu.a", "alpha", false, Vec::new(), "https://s/i.json"),
                listed("eu.b", "Beta", false, Vec::new(), "https://s/i.json"),
            ],
        );
        let result = buckets(&state, &langs(&["en"]), true, false);
        let names: Vec<&str> = result
            .installed
            .iter()
            .map(|ext| ext.pkg.as_str())
            .collect();
        assert_eq!(names, ["eu.gone", "eu.a", "eu.b"]);
    }

    #[test]
    fn packages_with_an_update_go_to_the_updates_bucket() {
        let state = state(
            vec![
                installed("eu.a", "A", false, true),
                installed("eu.b", "B", false, false),
            ],
            Vec::new(),
        );
        let result = buckets(&state, &langs(&["en"]), true, false);
        assert_eq!(
            result
                .updates
                .iter()
                .map(|ext| ext.pkg.as_str())
                .collect::<Vec<_>>(),
            ["eu.a"]
        );
        assert_eq!(
            result
                .installed
                .iter()
                .map(|ext| ext.pkg.as_str())
                .collect::<Vec<_>>(),
            ["eu.b"]
        );
    }

    #[test]
    fn tst_19_067_an_available_package_with_no_sources_is_one_row_without_a_source() {
        let state = state(
            Vec::new(),
            vec![listed(
                "eu.empty",
                "Empty",
                false,
                Vec::new(),
                "https://s/i.json",
            )],
        );
        let result = buckets(&state, &langs(&["en"]), true, false);
        assert_eq!(result.available.len(), 1);
        assert_eq!(result.available[0].source, None);
        assert_eq!(result.available[0].row_key, "eu.empty");
    }

    #[test]
    fn tst_19_068_two_sources_in_an_enabled_language_are_two_rows_with_split_keys() {
        let sources = vec![source(11, "One", "en"), source(22, "Two", "en")];
        let state = state(
            Vec::new(),
            vec![listed(
                "eu.split",
                "Split",
                false,
                sources,
                "https://s/i.json",
            )],
        );
        let result = buckets(&state, &langs(&["en"]), true, false);
        let keys: Vec<&str> = result
            .available
            .iter()
            .map(|row| row.row_key.as_str())
            .collect();
        assert_eq!(keys, ["eu.split-11", "eu.split-22"]);
        for row in &result.available {
            assert_eq!(
                row.ext.pkg, "eu.split",
                "the install key is the bare package name"
            );
        }
    }

    #[test]
    fn sources_in_a_disabled_language_give_no_row() {
        let sources = vec![source(11, "One", "fr"), source(22, "Two", "en")];
        let state = state(
            Vec::new(),
            vec![listed(
                "eu.mixed",
                "Mixed",
                false,
                sources,
                "https://s/i.json",
            )],
        );
        let result = buckets(&state, &langs(&["en"]), true, false);
        assert_eq!(result.available.len(), 1);
        assert_eq!(result.available[0].row_key, "eu.mixed-22");
    }

    #[test]
    fn tst_19_069_an_untrusted_nsfw_package_is_shown_even_when_nsfw_is_off() {
        let untrusted = Untrusted {
            name: "Adult".into(),
            pkg: "eu.adult".into(),
            version_name: "1.0.0".into(),
            version_code: 1,
            abi: AbiVersion { major: 1, minor: 0 },
            nsfw: true,
            signer: SignerDigest("cd".repeat(32)),
        };
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![LoadResult::Untrusted(untrusted)]);
        let result = buckets(&state, &langs(&["en"]), false, false);
        assert_eq!(
            result
                .untrusted
                .iter()
                .map(|ext| ext.pkg.as_str())
                .collect::<Vec<_>>(),
            ["eu.adult"]
        );
    }

    #[test]
    fn tst_19_070_the_nsfw_setting_is_read_on_each_rebuild() {
        let state = state(
            Vec::new(),
            vec![listed(
                "eu.adult",
                "Adult",
                true,
                vec![source(1, "A", "en")],
                "https://s/i.json",
            )],
        );
        assert!(buckets(&state, &langs(&["en"]), false, false).available.is_empty());
        assert_eq!(buckets(&state, &langs(&["en"]), true, false).available.len(), 1);
        assert!(buckets(&state, &langs(&["en"]), false, false).available.is_empty());
    }

    #[test]
    fn an_installed_or_untrusted_package_is_not_listed_as_available() {
        let untrusted = Untrusted {
            name: "U".into(),
            pkg: "eu.u".into(),
            version_name: "1.0.0".into(),
            version_code: 1,
            abi: AbiVersion { major: 1, minor: 0 },
            nsfw: false,
            signer: SignerDigest("cd".repeat(32)),
        };
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![
            LoadResult::Success(installed("eu.a", "A", false, false)),
            LoadResult::Untrusted(untrusted),
        ]);
        state.apply_available(vec![
            listed(
                "eu.a",
                "A",
                false,
                vec![source(1, "A", "en")],
                "https://s/i.json",
            ),
            listed(
                "eu.u",
                "U",
                false,
                vec![source(2, "U", "en")],
                "https://s/i.json",
            ),
            listed(
                "eu.new",
                "New",
                false,
                vec![source(3, "N", "en")],
                "https://s/i.json",
            ),
        ]);
        let result = buckets(&state, &langs(&["en"]), true, false);
        assert_eq!(
            result
                .available
                .iter()
                .map(|row| row.ext.pkg.as_str())
                .collect::<Vec<_>>(),
            ["eu.new"]
        );
    }

    #[test]
    fn t_19a_066_not_runnable_rows_are_hidden_and_counted_unless_asked_for() {
        let mut apk = listed("eu.apk", "Apk", false, vec![source(1, "S", "en")], "https://a/i.json");
        apk.kind = Kind::ApkOnly;
        let mut unsigned = listed("eu.unsigned", "Unsigned", false, vec![], "https://a/i.json");
        unsigned.kind = Kind::Unsigned;
        let mut state = ExtensionState::new();
        state.apply_available(vec![apk, unsigned, listed("eu.ok", "Ok", false, vec![], "https://a/i.json")]);
        let hidden = buckets(&state, &langs(&["en"]), true, false);
        assert_eq!(hidden.not_runnable_hidden, 2);
        assert_eq!(hidden.available.iter().map(|r| r.ext.pkg.as_str()).collect::<Vec<_>>(), ["eu.ok"]);
        let shown = buckets(&state, &langs(&["en"]), true, true);
        assert_eq!(shown.not_runnable_hidden, 0);
        assert_eq!(shown.available.len(), 3, "the flag shows every runnable and not-runnable row");
    }

    #[test]
    fn t_19a_066_badges_follow_the_kind_and_disable_install() {
        assert_eq!(RowBadges::of(Kind::ApkOnly), RowBadges { not_runnable: true, unsigned: false });
        assert_eq!(RowBadges::of(Kind::Unsigned), RowBadges { not_runnable: false, unsigned: true });
        assert!(RowBadges::of(Kind::Wasm).install_enabled());
        assert!(!RowBadges::of(Kind::ApkOnly).install_enabled());
        assert!(!RowBadges::of(Kind::Unsigned).install_enabled());
    }

    #[test]
    fn t_19a_066_hidden_entries_are_counted_per_store() {
        let mut apk = listed("eu.apk", "Apk", false, vec![], "https://a/i.json");
        apk.kind = Kind::ApkOnly;
        let mut unsigned = listed("eu.unsigned", "Unsigned", false, vec![], "https://b/i.json");
        unsigned.kind = Kind::Unsigned;
        let mut state = ExtensionState::new();
        state.apply_available(vec![apk, unsigned, listed("eu.ok", "Ok", false, vec![], "https://a/i.json")]);
        let counts = not_runnable_by_store(&state, false);
        assert_eq!(counts.get("https://a/i.json"), Some(&1));
        assert_eq!(counts.get("https://b/i.json"), Some(&1));
        assert!(not_runnable_by_store(&state, true).is_empty(), "nothing is hidden when the flag is on");
    }

    #[test]
    fn a_duplicate_package_uses_its_first_listing() {
        let first = listed(
            "eu.dup",
            "Dup",
            false,
            vec![source(1, "First", "en")],
            "https://one/i.json",
        );
        let second = listed(
            "eu.dup",
            "Dup",
            false,
            vec![source(2, "Second", "en")],
            "https://two/i.json",
        );
        let state = state(Vec::new(), vec![first, second]);
        let result = buckets(&state, &langs(&["en"]), true, false);
        assert_eq!(result.available.len(), 1);
        assert_eq!(
            result.available[0].ext.store.index_url,
            "https://one/i.json"
        );
    }
}
