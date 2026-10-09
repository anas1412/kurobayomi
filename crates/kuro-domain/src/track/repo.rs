//! The track repository port (`MangaTrackRepository.kt`, `AnimeTrackRepository.kt`). One trait for both kinds; the
//! SQL is in `kuro-data::track`, which implements it below.

use crate::error::DomainError;
use crate::source::model::Id;
use crate::track::{AnimeTrack, MangaTrack};

/// Reads and writes of one track kind. `by_row_id` takes a row id, never an entry id (Q-TRK-1). The live lists
/// (`watch_all`, `watch_by_entry`) are methods of the concrete `kuro-data` repositories: a `Subscription` needs a
/// tokio runtime, and this trait is also implemented by in-memory fakes in tests.
pub trait TrackRepo<T>: Send + Sync {
    fn by_row_id(&self, row_id: Id) -> Result<Option<T>, DomainError>;
    fn by_entry(&self, entry_id: Id) -> Result<Vec<T>, DomainError>;
    fn all(&self) -> Result<Vec<T>, DomainError>;
    /// Upserts by (entry, tracker): the row id stays the same on a second write (Q-TRK-2).
    fn insert(&self, track: &T) -> Result<(), DomainError>;
    /// One transaction; a repeated pair leaves one row with the last values (TC-TRK-06).
    fn insert_all(&self, tracks: &[T]) -> Result<(), DomainError>;
    /// Removes the binding of `entry_id` to `tracker_id`. Zero rows affected is `Ok` (TC-TRK-03).
    fn delete(&self, entry_id: Id, tracker_id: Id) -> Result<(), DomainError>;
}

impl TrackRepo<MangaTrack> for kuro_data::track::MangaTrackRepo {
    fn by_row_id(&self, row_id: Id) -> Result<Option<MangaTrack>, DomainError> {
        Ok(kuro_data::track::MangaTrackRepo::by_id(self, row_id)?)
    }
    fn by_entry(&self, entry_id: Id) -> Result<Vec<MangaTrack>, DomainError> {
        Ok(kuro_data::track::MangaTrackRepo::by_entry(self, entry_id)?)
    }
    fn all(&self) -> Result<Vec<MangaTrack>, DomainError> {
        Ok(kuro_data::track::MangaTrackRepo::all(self)?)
    }
    fn insert(&self, track: &MangaTrack) -> Result<(), DomainError> {
        Ok(kuro_data::track::MangaTrackRepo::insert(self, track).map(drop)?)
    }
    fn insert_all(&self, tracks: &[MangaTrack]) -> Result<(), DomainError> {
        Ok(kuro_data::track::MangaTrackRepo::insert_all(self, tracks)?)
    }
    fn delete(&self, entry_id: Id, tracker_id: Id) -> Result<(), DomainError> {
        Ok(kuro_data::track::MangaTrackRepo::delete(self, entry_id, tracker_id)?)
    }
}

impl TrackRepo<AnimeTrack> for kuro_data::track::AnimeTrackRepo {
    fn by_row_id(&self, row_id: Id) -> Result<Option<AnimeTrack>, DomainError> {
        Ok(kuro_data::track::AnimeTrackRepo::by_id(self, row_id)?)
    }
    fn by_entry(&self, entry_id: Id) -> Result<Vec<AnimeTrack>, DomainError> {
        Ok(kuro_data::track::AnimeTrackRepo::by_entry(self, entry_id)?)
    }
    fn all(&self) -> Result<Vec<AnimeTrack>, DomainError> {
        Ok(kuro_data::track::AnimeTrackRepo::all(self)?)
    }
    fn insert(&self, track: &AnimeTrack) -> Result<(), DomainError> {
        Ok(kuro_data::track::AnimeTrackRepo::insert(self, track).map(drop)?)
    }
    fn insert_all(&self, tracks: &[AnimeTrack]) -> Result<(), DomainError> {
        Ok(kuro_data::track::AnimeTrackRepo::insert_all(self, tracks)?)
    }
    fn delete(&self, entry_id: Id, tracker_id: Id) -> Result<(), DomainError> {
        Ok(kuro_data::track::AnimeTrackRepo::delete(self, entry_id, tracker_id)?)
    }
}
