//! The few texts of the Browse tab that the string catalogue does not hold yet. They are Proposals of the plan
//! (T-41a-027, T-41a-036) that replace an upstream text bug or add a hint, and the catalogue crate is not part of this
//! lane, so they live here as constants until `kuro-i18n` takes them: each constant names the key it will become.

/// `browse_global_search_empty_hint`: the hint of an empty global search (T-41a-027, ui-parity S4).
pub const GLOBAL_SEARCH_EMPTY_HINT: &str = "Search a source by title";

/// `migrate_sources_empty`: the empty picker of the anime migration, which upstream words as "Your library is empty"
/// (a text bug; T-41a-036, ui-parity S9).
pub const MIGRATE_SOURCES_EMPTY: &str = "No source has favorites to migrate";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_texts_are_the_ones_the_plan_words() {
        assert_eq!(GLOBAL_SEARCH_EMPTY_HINT, "Search a source by title");
        assert_eq!(MIGRATE_SOURCES_EMPTY, "No source has favorites to migrate");
    }
}
