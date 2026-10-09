//! Tracker bindings (area 09 section 1, `track/`). Models come from `kuro-data`; this module adds the repository port,
//! the interactors with the log-and-default policy, and the per-entry grouping.

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
