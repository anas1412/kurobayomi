//! Build-variant flags (Kotlin `BuildConfig.kt`, T-52a-034).
//!
//! `IS_PREVIEW_BUILD` is not here: the `preview` profile is reported by `kuro_core::app_info`, which owns the build type.

/// The in-app updater is dropped; the package manager or a source build updates the app (plans 52 overview, decision 3).
/// This is the value the util layer reports in every build. The `updater` Cargo feature belongs to `kuro-core`.
pub const UPDATER_ENABLED: bool = false;

/// True in a debug build.
pub const IS_DEBUG_BUILD: bool = cfg!(debug_assertions);

/// True in a release build: the negation of [`IS_DEBUG_BUILD`].
pub const IS_RELEASE_BUILD: bool = !IS_DEBUG_BUILD;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_updater_is_off_in_every_build() {
        // TC-34-a
        assert!(!UPDATER_ENABLED);
    }

    #[test]
    fn debug_follows_debug_assertions_and_release_is_its_negation() {
        assert_eq!(IS_DEBUG_BUILD, cfg!(debug_assertions));
        assert_eq!(IS_RELEASE_BUILD, !IS_DEBUG_BUILD);
    }
}
