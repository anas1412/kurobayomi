//! Episode actions that are pure decisions: bulk bookmark and fillermark (T-39b-015) and the open routing of an episode
//! (T-39b-017). Upstream: `AnimeScreenModel.kt` (`onMultiBookmarkClicked`, `onMultiFillermarkClicked`) and
//! `AnimeScreen.kt:136-144`. Behaviour 11.2 and 17.4 and 17.5. The caller writes the flags and clears the selection.

use kuro_data::episode::Episode;

/// The per-episode flags a bulk action sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Bookmark,
    Fillermark,
}

fn flag(episode: &Episode, mark: Mark) -> bool {
    match mark {
        Mark::Bookmark => episode.bookmark,
        Mark::Fillermark => episode.fillermark,
    }
}

/// The ids to write `on` for. Only episodes whose flag differs are written (behaviour 11.2); an episode already at
/// `on` is left out, so a bulk call never writes a row that does not change.
pub fn bulk_mark_targets(selected: &[Episode], mark: Mark, on: bool) -> Vec<i64> {
    selected.iter().filter(|e| flag(e, mark) != on).map(|e| e.id).collect()
}

/// A tap opens the episode externally when the setting says so, and `alt` inverts it (behaviour 17.4).
pub fn tap_opens_external(always_external: bool, alt: bool) -> bool {
    always_external ^ alt
}

/// Continue watching and the season continue button use the setting as it is, with no inversion (17.2, 17.3).
pub fn continue_opens_external(always_external: bool) -> bool {
    always_external
}

/// The torrent server starts before the player opens, and only for a torrent source (17.5).
pub fn starts_torrent_server(is_torrent: bool) -> bool {
    is_torrent
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: i64, bookmark: bool, fillermark: bool) -> Episode {
        let mut e = Episode::create();
        e.id = id;
        e.bookmark = bookmark;
        e.fillermark = fillermark;
        e
    }

    #[test]
    fn bulk_bookmark_writes_only_the_episodes_that_differ() {
        let list = [ep(1, false, false), ep(2, true, false), ep(3, false, true)];
        assert_eq!(bulk_mark_targets(&list, Mark::Bookmark, true), vec![1, 3]);
        assert_eq!(bulk_mark_targets(&list, Mark::Bookmark, false), vec![2]);
    }

    #[test]
    fn bulk_fillermark_uses_its_own_flag() {
        let list = [ep(1, true, false), ep(2, false, true)];
        assert_eq!(bulk_mark_targets(&list, Mark::Fillermark, true), vec![1]);
        assert_eq!(bulk_mark_targets(&list, Mark::Fillermark, false), vec![2]);
    }

    #[test]
    fn nothing_to_write_when_every_flag_already_matches() {
        assert!(bulk_mark_targets(&[ep(1, true, false)], Mark::Bookmark, true).is_empty());
        assert!(bulk_mark_targets(&[], Mark::Bookmark, true).is_empty());
    }

    #[test]
    fn tap_inverts_the_default_with_alt() {
        assert!(!tap_opens_external(false, false), "internal by default");
        assert!(tap_opens_external(false, true), "alt opens external");
        assert!(tap_opens_external(true, false), "external by default");
        assert!(!tap_opens_external(true, true), "alt opens internal");
    }

    #[test]
    fn continue_never_inverts() {
        assert!(continue_opens_external(true));
        assert!(!continue_opens_external(false));
    }

    #[test]
    fn torrent_server_only_for_torrent_sources() {
        assert!(starts_torrent_server(true));
        assert!(!starts_torrent_server(false));
    }
}
