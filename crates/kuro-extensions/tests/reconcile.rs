//! Edge cases 12.4 (T-18a-044): reconciling installed packages with the store listing (sections 3 and 4, X11, X14, X15,
//! U9, U10). The "lib version" of the manga rule is the package ABI here, which a listing does not carry until a
//! package is downloaded, so U9 is tested with an ABI set on the listing by hand.

mod common;

use common::{installed, listed};
use kuro_extensions::anime::abi::AbiVersion;
use kuro_extensions::anime::manager::ExtensionState;
use kuro_extensions::anime::model::LoadResult;

fn state_with(installed_packages: &[(&str, i64)]) -> ExtensionState {
    let mut state = ExtensionState::new();
    state.apply_loaded(installed_packages.iter().map(|(pkg, code)| LoadResult::Success(installed(pkg, pkg, *code))).collect());
    state
}

#[test]
fn obsolete_marked_when_missing() {
    let mut state = state_with(&[("p.gone", 1), ("p.kept", 1)]);
    state.apply_available(vec![listed("p.kept", "Kept", 1)]);
    assert!(state.installed()["p.gone"].is_obsolete);
    assert!(!state.installed()["p.gone"].has_update, "an obsolete package has no update");
    assert!(!state.installed()["p.kept"].is_obsolete);
}

#[test]
fn obsolete_reset_when_reappears() {
    let mut state = state_with(&[("p.back", 1)]);
    state.apply_available(vec![listed("p.other", "Other", 1)]);
    assert!(state.installed()["p.back"].is_obsolete);
    state.apply_available(vec![listed("p.other", "Other", 1), listed("p.back", "Back", 1)]);
    assert!(!state.installed()["p.back"].is_obsolete, "a package that is listed again is not obsolete");
    assert_eq!(state.installed()["p.back"].store, Some(common::store_ref()));
}

#[test]
fn empty_available_keeps_installed_and_zeroes_count() {
    let mut state = state_with(&[("p.a", 1), ("p.b", 1)]);
    state.apply_available(vec![listed("p.a", "A", 5)]);
    assert_eq!((state.update_count(), state.installed()["p.b"].is_obsolete), (1, true));
    state.apply_available(Vec::new());
    assert_eq!(state.installed().len(), 2, "installed packages stay");
    assert_eq!(state.update_count(), 0, "no listing, no updates");
}

#[test]
fn has_update_by_version_or_lib() {
    let mut state = state_with(&[("p.newer", 1), ("p.same", 5), ("p.lib", 5), ("p.older", 5)]);
    let mut lib = listed("p.lib", "Lib", 5);
    lib.abi = Some(AbiVersion { major: 1, minor: 1 });
    state.apply_available(vec![listed("p.newer", "Newer", 2), listed("p.same", "Same", 5), lib, listed("p.older", "Older", 4)]);
    let has_update = |pkg: &str| state.installed()[pkg].has_update;
    assert!(has_update("p.newer"), "a higher version code");
    assert!(!has_update("p.same"), "an equal version code");
    assert!(has_update("p.lib"), "a higher ABI alone (U9)");
    assert!(!has_update("p.older"), "a listing with a lower version code is not an update (U10)");
    assert_eq!(state.update_count(), 2);
}
