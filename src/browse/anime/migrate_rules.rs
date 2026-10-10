//! The anime migrate fetch-type rules (T-41a-041). A title can move only to a title that fetches the same way: episodes to
//! episodes, seasons to seasons. A mismatch still lets the user open the search results ("Show anime" and "Show seasons"),
//! but migrating is off.

use kuro_source::anime::model::FetchType;

/// Whether `old` can migrate to `new` (behavior 11.1).
pub fn can_migrate(old: FetchType, new: FetchType) -> bool {
    old == new
}

/// "Show seasons" is offered iff the new title is not an episodes title.
pub fn show_seasons(new: FetchType) -> bool {
    new != FetchType::Episodes
}

/// The episodes flag is offered only when the old title fetches episodes (behavior 10).
pub fn episodes_flag_visible(old: FetchType) -> bool {
    old == FetchType::Episodes
}

/// What the migrate dialog offers for an old and a new title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    pub migrate: bool,
    pub show_seasons: bool,
    pub episodes_flag: bool,
}

pub fn offer(old: FetchType, new: FetchType) -> Offer {
    Offer { migrate: can_migrate(old, new), show_seasons: show_seasons(new), episodes_flag: episodes_flag_visible(old) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrating_needs_the_same_fetch_type_on_both_sides() {
        assert!(can_migrate(FetchType::Episodes, FetchType::Episodes));
        assert!(can_migrate(FetchType::Seasons, FetchType::Seasons));
        assert!(!can_migrate(FetchType::Episodes, FetchType::Seasons), "E41: a mismatch cannot migrate");
        assert!(!can_migrate(FetchType::Seasons, FetchType::Episodes), "E42: the rule is symmetric in refusing");
    }

    #[test]
    fn show_seasons_is_offered_for_every_new_title_that_is_not_episodes() {
        assert!(show_seasons(FetchType::Seasons));
        assert!(!show_seasons(FetchType::Episodes));
    }

    #[test]
    fn the_episodes_flag_is_shown_only_for_an_episodes_title() {
        assert!(episodes_flag_visible(FetchType::Episodes));
        assert!(!episodes_flag_visible(FetchType::Seasons));
    }

    #[test]
    fn a_mismatch_keeps_the_search_offers_and_turns_migrating_off() {
        let mismatch = offer(FetchType::Episodes, FetchType::Seasons);
        assert_eq!(mismatch, Offer { migrate: false, show_seasons: true, episodes_flag: true });
        assert!(offer(FetchType::Seasons, FetchType::Seasons).migrate);
    }
}
