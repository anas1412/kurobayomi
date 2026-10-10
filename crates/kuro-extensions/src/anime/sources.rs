//! Source toggles and the label rule (T-19a-029, `GetAnimeExtensionSources.kt`). A source is enabled when its decimal
//! id is not in `hidden_anime_catalogues`. Ids stay `i64`. The browse list is rebuilt from the preference on every
//! call, so a toggle shows on the next rebuild.

use std::collections::BTreeSet;
use std::path::PathBuf;

use kuro_prefs::PrefError;
use kuro_prefs::source::SourcePrefs;
use kuro_source::local::LOCAL_SOURCE_ID;

use crate::anime::manager::ExtensionState;
use crate::anime::model::{AnimeSourceRef, Installed};

/// `source.id.toString() !in hidden`.
pub fn is_enabled(id: i64, hidden: &BTreeSet<String>) -> bool {
    !hidden.contains(&id.to_string())
}

/// The name label shows when the package has several sources and their names are not all equal. Otherwise the language
/// label shows.
pub fn label_as_name(sources: &[AnimeSourceRef]) -> bool {
    sources.len() > 1 && sources.iter().skip(1).any(|source| source.name != sources[0].name)
}

/// One source row of an extension's details page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceItem<'a> {
    pub source: &'a AnimeSourceRef,
    pub enabled: bool,
    pub label_as_name: bool,
}

pub fn source_items<'a>(extension: &'a Installed, hidden: &BTreeSet<String>) -> Vec<SourceItem<'a>> {
    let label_as_name = label_as_name(&extension.sources);
    extension
        .sources
        .iter()
        .map(|source| SourceItem { source, enabled: is_enabled(source.id, hidden), label_as_name })
        .collect()
}

/// The installed package that owns `source`. There is none for no source, for the local source, and for a stub (a
/// source whose package is not installed).
fn owner<'a>(state: &'a ExtensionState, source: Option<&AnimeSourceRef>) -> Option<&'a Installed> {
    let source = source.filter(|source| source.id != LOCAL_SOURCE_ID)?;
    state.installed().values().find(|installed| installed.sources.iter().any(|owned| owned.id == source.id))
}

/// `isSourceForTorrents` (T-17a-033): the owning extension's `is_torrent` flag. False for no source, the local source,
/// and a missing owner.
pub fn is_source_for_torrents(state: &ExtensionState, source: Option<&AnimeSourceRef>) -> bool {
    owner(state, source).is_some_and(|installed| installed.torrent)
}

/// The icon of the owning package (T-17a-025). The icon was read when the package loaded, so the package keeps one
/// entry, which is the per-package cache. `None` for no source, the local source, a stub, or a package without an icon
/// file; the UI then draws the generic icon.
pub fn icon_for_source(state: &ExtensionState, source: Option<&AnimeSourceRef>) -> Option<PathBuf> {
    owner(state, source)?.icon.clone()
}

/// Flips one source and returns whether it is enabled now. The caller then calls [`browse_sources`] again.
pub fn toggle_source(prefs: &SourcePrefs, id: i64) -> Result<bool, PrefError> {
    let mut hidden = prefs.disabled_anime_sources.get();
    let key = id.to_string();
    let was_hidden = hidden.remove(&key);
    if !was_hidden {
        hidden.insert(key);
    }
    prefs.disabled_anime_sources.set(hidden)?;
    Ok(was_hidden)
}

/// The enabled sources of the installed packages, in package order. It reads the preference now, so it is the rebuild
/// that follows a toggle.
pub fn browse_sources<'a>(state: &'a ExtensionState, prefs: &SourcePrefs) -> Vec<&'a AnimeSourceRef> {
    let hidden = prefs.disabled_anime_sources.get();
    state
        .installed()
        .values()
        .flat_map(|installed| installed.sources.iter())
        .filter(|source| is_enabled(source.id, &hidden))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::abi::AbiVersion;
    use crate::anime::model::{LoadResult, SignerDigest};
    use kuro_prefs::store::PrefStore;

    fn source(id: i64, name: &str) -> AnimeSourceRef {
        AnimeSourceRef { id, name: name.into(), lang: "en".into(), base_url: String::new() }
    }

    fn installed(pkg: &str, sources: Vec<AnimeSourceRef>) -> Installed {
        Installed {
            name: pkg.into(),
            pkg: pkg.into(),
            version_name: "1".into(),
            version_code: 1,
            abi: AbiVersion { major: 1, minor: 0 },
            lang: "en".into(),
            nsfw: false,
            torrent: false,
            sources,
            icon: None,
            signer: SignerDigest("ab".repeat(32)),
            has_update: false,
            is_obsolete: false,
            store: None,
        }
    }

    #[test]
    fn tst_19_024_a_large_id_is_matched_as_its_decimal_string() {
        let id = 9_007_199_254_740_993_i64; // 2^53 + 1, which a float round trip would change
        let hidden: BTreeSet<String> = ["9007199254740993".to_owned()].into();
        assert!(!is_enabled(id, &hidden));
        assert!(is_enabled(id - 1, &hidden));
        assert!(is_enabled(-5, &BTreeSet::new()));
    }

    #[test]
    fn tst_19_074_the_label_is_the_name_only_for_several_sources_with_different_names() {
        assert!(!label_as_name(&[]));
        assert!(!label_as_name(&[source(1, "A")]), "one source: language label");
        assert!(!label_as_name(&[source(1, "A"), source(2, "A")]), "several sources, one name: language label");
        assert!(label_as_name(&[source(1, "A"), source(2, "B")]));
    }

    #[test]
    fn source_items_carry_the_enabled_flag_and_the_label_rule() {
        let ext = installed("p.a", vec![source(1, "A"), source(2, "B")]);
        let hidden: BTreeSet<String> = ["2".to_owned()].into();
        let items = source_items(&ext, &hidden);
        assert_eq!(items.iter().map(|item| item.enabled).collect::<Vec<_>>(), [true, false]);
        assert!(items.iter().all(|item| item.label_as_name));
    }

    fn torrent_package(pkg: &str, sources: Vec<AnimeSourceRef>, torrent: bool) -> Installed {
        Installed { torrent, ..installed(pkg, sources) }
    }

    #[test]
    fn a_torrent_source_takes_the_flag_of_its_owning_package() {
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![
            LoadResult::Success(torrent_package("p.torrent", vec![source(10, "T")], true)),
            LoadResult::Success(torrent_package("p.plain", vec![source(11, "P")], false)),
        ]);
        assert!(is_source_for_torrents(&state, Some(&source(10, "T"))));
        assert!(!is_source_for_torrents(&state, Some(&source(11, "P"))));
    }

    #[test]
    fn no_source_local_source_and_a_stub_are_never_torrent_sources() {
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![LoadResult::Success(torrent_package("p.torrent", vec![source(10, "T")], true))]);
        assert!(!is_source_for_torrents(&state, None));
        assert!(!is_source_for_torrents(&state, Some(&source(LOCAL_SOURCE_ID, "Local"))));
        assert!(!is_source_for_torrents(&state, Some(&source(99, "Stub, package not installed"))), "a missing owner is false");
    }

    #[test]
    fn the_icon_is_the_owning_packages_icon_and_none_for_stubs_and_local() {
        let icon = PathBuf::from("/cache/ext-icons/p.torrent.png");
        let mut with_icon = torrent_package("p.torrent", vec![source(10, "T")], true);
        with_icon.icon = Some(icon.clone());
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![LoadResult::Success(with_icon), LoadResult::Success(installed("p.none", vec![source(12, "N")]))]);
        assert_eq!(icon_for_source(&state, Some(&source(10, "T"))), Some(icon));
        assert_eq!(icon_for_source(&state, Some(&source(12, "N"))), None, "no icon file: the generic icon");
        assert_eq!(icon_for_source(&state, Some(&source(99, "Stub"))), None);
        assert_eq!(icon_for_source(&state, Some(&source(LOCAL_SOURCE_ID, "Local"))), None);
        assert_eq!(icon_for_source(&state, None), None);
    }

    #[test]
    fn toggling_a_source_changes_the_next_browse_rebuild() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![LoadResult::Success(installed("p.a", vec![source(1, "A"), source(2, "B")]))]);
        let ids = |state: &ExtensionState| browse_sources(state, &prefs).iter().map(|s| s.id).collect::<Vec<_>>();
        assert_eq!(ids(&state), [1, 2]);
        assert!(!toggle_source(&prefs, 1).unwrap(), "now disabled");
        assert_eq!(ids(&state), [2]);
        assert_eq!(prefs.disabled_anime_sources.get(), BTreeSet::from(["1".to_owned()]));
        assert!(toggle_source(&prefs, 1).unwrap(), "enabled again");
        assert_eq!(ids(&state), [1, 2]);
    }
}
