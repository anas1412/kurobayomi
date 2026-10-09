//! Episode switching and autoplay (area 45 `PlayerActivity.changeEpisode` and `endFile`, behaviour
//! 7.1-7.2; T-45a-036, T-45a-037). The mpv pause and the episode load are the caller's commands.
//! This module changes the state and returns the decisions.

use crate::enums::{Panels, Sheets};
use crate::errors::{InitError, PlayerMessage};
use crate::state::PlayerState;

/// `hasPreviousEpisode = index != 0` and `hasNextEpisode = index != size - 1`. An index of `-1`
/// (not found) counts as having both neighbours, as in Kotlin.
pub fn neighbour_flags(index: i64, len: usize) -> (bool, bool) {
    (index != 0, index != len as i64 - 1)
}

/// `endFile(eofReached)`: autoplay runs only at the real end of the file, with the preference on.
pub fn autoplay_on_eof(eof_reached: bool, autoplay_enabled: bool) -> bool {
    eof_reached && autoplay_enabled
}

/// The state part of `changeEpisode` before the load (PlayerActivity.kt:1002-1010): the sheet and
/// the panel close, loading starts, and the episode and hoster loading flags are set. The caller
/// pauses mpv first.
pub fn begin_switch(state: &mut PlayerState) {
    state.sheet = Sheets::None;
    state.panel = Panels::None;
    state.is_loading = true;
    state.is_loading_episode = true;
    state.is_loading_hosters = true;
}

/// The "Next Episode not found!" toast (behaviour 7.1 step e). It shows only when there is an
/// anime, the switch found no episode, and autoplay is off.
pub fn no_next_episode_message(
    has_anime: bool,
    switch_found: bool,
    auto_play: bool,
) -> Option<PlayerMessage> {
    (has_anime && !switch_found && !auto_play).then_some(PlayerMessage::NoNextEpisode)
}

/// What to do with the hoster list of a switched episode (behaviour 7.1 step f).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwitchHosters {
    /// Load the hosters with no preselected index.
    Load,
    /// The list is empty. The window closes through the initial-error path.
    Fail(InitError),
    /// The episode has no hoster list at all. Nothing loads ("Error getting links").
    NoLinks,
}

/// Classifies a switched episode's hoster list. `None` means the list is missing (`null`).
pub fn switch_hosters(hoster_count: Option<usize>) -> SwitchHosters {
    match hoster_count {
        None => SwitchHosters::NoLinks,
        Some(0) => SwitchHosters::Fail(InitError::Localised(PlayerMessage::NoHosters)),
        Some(_) => SwitchHosters::Load,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_and_last_have_one_neighbour_only() {
        assert_eq!(neighbour_flags(0, 3), (false, true));
        assert_eq!(neighbour_flags(2, 3), (true, false));
        assert_eq!(neighbour_flags(1, 3), (true, true));
        assert_eq!(neighbour_flags(0, 1), (false, false));
    }

    #[test]
    fn missing_index_counts_as_both_neighbours() {
        assert_eq!(neighbour_flags(-1, 3), (true, true));
    }

    #[test]
    fn autoplay_needs_eof_and_preference() {
        assert!(autoplay_on_eof(true, true));
        assert!(!autoplay_on_eof(true, false));
        assert!(!autoplay_on_eof(false, true));
    }

    #[test]
    fn begin_switch_closes_panels_and_sets_loading() {
        let mut state = PlayerState {
            sheet: Sheets::More,
            panel: Panels::None,
            ..PlayerState::default()
        };
        begin_switch(&mut state);
        assert_eq!(state.sheet, Sheets::None);
        assert_eq!(state.panel, Panels::None);
        assert!(state.is_loading);
        assert!(state.is_loading_episode);
        assert!(state.is_loading_hosters);
    }

    #[test]
    fn next_episode_toast_only_without_autoplay() {
        assert_eq!(
            no_next_episode_message(true, false, false),
            Some(PlayerMessage::NoNextEpisode)
        );
        assert_eq!(no_next_episode_message(true, false, true), None);
        assert_eq!(no_next_episode_message(true, true, false), None);
        assert_eq!(no_next_episode_message(false, false, false), None);
    }

    #[test]
    fn empty_hoster_list_is_an_init_error() {
        assert_eq!(
            switch_hosters(Some(0)),
            SwitchHosters::Fail(InitError::Localised(PlayerMessage::NoHosters))
        );
        assert_eq!(switch_hosters(Some(2)), SwitchHosters::Load);
        assert_eq!(switch_hosters(None), SwitchHosters::NoLinks);
    }
}
