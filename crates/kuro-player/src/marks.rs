//! Bookmark and fillermark of the playing episode (area 45 `PlayerViewModel.bookmarkEpisode` and
//! `fillermarkEpisode`, behaviour 7.10; T-45b-040). The database update and the in-memory episode
//! change together, so the next progress save does not write the stale value back (E-15). A missing
//! episode id is a logged no-op instead of a crash (`episodeId!!` in Kotlin).

/// Which mark changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Bookmark,
    Fillermark,
}

/// The playing episode as the player holds it in memory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CurrentEpisode {
    pub id: Option<i64>,
    pub bookmark: bool,
    pub fillermark: bool,
}

/// The database write for one mark (`EpisodeUpdate` with one field set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkUpdate {
    pub episode_id: i64,
    pub mark: Mark,
    pub value: bool,
}

/// Sets `mark` on the in-memory episode and returns the database write for it. Returns `None` when
/// the episode has no id; nothing changes then.
pub fn set_mark(episode: &mut CurrentEpisode, mark: Mark, value: bool) -> Option<MarkUpdate> {
    let Some(episode_id) = episode.id else {
        tracing::error!(
            ?mark,
            "bookmark or fillermark without an episode id; ignored"
        );
        return None;
    };
    match mark {
        Mark::Bookmark => episode.bookmark = value,
        Mark::Fillermark => episode.fillermark = value,
    }
    Some(MarkUpdate {
        episode_id,
        mark,
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t62_a_mark_updates_the_memory_and_returns_the_write() {
        let mut episode = CurrentEpisode {
            id: Some(42),
            ..CurrentEpisode::default()
        };
        assert_eq!(
            set_mark(&mut episode, Mark::Bookmark, true),
            Some(MarkUpdate {
                episode_id: 42,
                mark: Mark::Bookmark,
                value: true,
            })
        );
        assert!(episode.bookmark);
        assert!(!episode.fillermark);

        set_mark(&mut episode, Mark::Fillermark, true);
        assert!(episode.fillermark);
        set_mark(&mut episode, Mark::Bookmark, false);
        assert!(!episode.bookmark);
    }

    #[test]
    fn a_missing_id_is_a_no_op() {
        let mut episode = CurrentEpisode::default();
        assert_eq!(set_mark(&mut episode, Mark::Bookmark, true), None);
        assert_eq!(episode, CurrentEpisode::default());
    }
}
