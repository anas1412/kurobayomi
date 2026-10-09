//! Tracker bindings (area 09 section 1, `track/`). Models come from `kuro-data`; this module adds the repository port,
//! the interactors with the log-and-default policy, and the per-entry grouping.

pub mod delayed;
pub mod repo;
pub mod service;

pub use kuro_data::track::{AnimeTrack, MangaTrack};

use crate::source::model::Id;

/// A track as the grouping helpers see it: the entry it belongs to.
pub trait Track {
    /// `manga_id` for a manga track, `anime_id` for an anime track.
    fn entry_id(&self) -> Id;
}

impl Track for MangaTrack {
    fn entry_id(&self) -> Id {
        self.manga_id
    }
}

impl Track for AnimeTrack {
    fn entry_id(&self) -> Id {
        self.anime_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_personal_from_copies_exactly_the_six_personal_fields_and_keeps_identity() {
        let src = MangaTrack {
            private: true,
            last_chapter_read: 3.0,
            score: 8.0,
            status: 2,
            start_date: 10,
            finish_date: 20,
            title: "other".into(),
            remote_id: 99,
            ..MangaTrack::new(1)
        };
        let mut dst = MangaTrack { id: 9, manga_id: 4, title: "keep".into(), ..MangaTrack::new(1) };
        dst.copy_personal_from(&src);
        assert_eq!(
            (dst.last_chapter_read, dst.score, dst.status, dst.start_date, dst.finish_date, dst.private),
            (3.0, 8.0, 2, 10, 20, true)
        );
        assert_eq!((dst.id, dst.manga_id, dst.remote_id, dst.title.as_str()), (9, 4, 0, "keep"));

        let asrc = AnimeTrack { private: true, last_episode_seen: 5.0, ..AnimeTrack::new(2) };
        let mut adst = AnimeTrack::new(2);
        adst.copy_personal_from(&asrc);
        assert_eq!((adst.private, adst.last_episode_seen), (true, 5.0));
    }
}
