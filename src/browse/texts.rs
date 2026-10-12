//! The few texts of the Browse tab that the string catalogue does not hold yet. They are Proposals of the plan
//! (T-41a-027, T-41a-036) that replace an upstream text bug or add a hint, and the catalogue crate is not part of this
//! lane, so they live here as constants until `kuro-i18n` takes them: each constant names the key it will become.

/// `browse_global_search_empty_hint`: the hint of an empty global search (T-41a-027, ui-parity S4).
pub const GLOBAL_SEARCH_EMPTY_HINT: &str = "Search a source by title";

/// `loading_sources`: the line under the spinner of a Sources tab while the repository sources load.
pub const LOADING_SOURCES: &str = "Loading sources…";

/// `migrate_sources_empty`: the empty picker of the anime migration, which upstream words as "Your library is empty"
/// (a text bug; T-41a-036, ui-parity S9).
pub const MIGRATE_SOURCES_EMPTY: &str = "No source has favorites to migrate";

/// `migrate_tracks_always`: the dim line under the migrate options. Tracking data is copied whatever is ticked (T-40a-045).
pub const MIGRATE_TRACKS_ALWAYS: &str = "Tracking data is always transferred.";

/// `migration_failed`: the inline error of a failed anime migration (T-41a-035, a Proposal over Aniyomi's silent path).
pub fn migration_failed(reason: &str) -> String {
    format!("Migration failed: {reason}")
}

/// The migrate tab's help action (T-41b-040, help URLs of `p2-data-and-config.md` section 5).
pub const SOURCE_MIGRATION_HELP_URL: &str = "https://aniyomi.org/help/guides/source-migration/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_texts_are_the_ones_the_plan_words() {
        assert_eq!(GLOBAL_SEARCH_EMPTY_HINT, "Search a source by title");
        assert_eq!(LOADING_SOURCES, "Loading sources…");
        assert_eq!(MIGRATE_SOURCES_EMPTY, "No source has favorites to migrate");
        assert_eq!(MIGRATE_TRACKS_ALWAYS, "Tracking data is always transferred.");
        assert_eq!(SOURCE_MIGRATION_HELP_URL, "https://aniyomi.org/help/guides/source-migration/");
        assert_eq!(migration_failed("disk full"), "Migration failed: disk full");
    }
}
