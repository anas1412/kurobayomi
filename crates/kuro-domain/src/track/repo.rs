//! The track repository port (`MangaTrackRepository.kt`, `AnimeTrackRepository.kt`). One trait for both kinds over the
//! shared `Track`; the SQL is `kuro_db::tables::tracks` in `kuro-data`, which implements it below.

use kuro_data::track::{Track, TrackRepo};
use kuro_platform::kind::Kind;

use crate::error::DomainError;
use crate::source::model::Id;

/// Reads and writes of track bindings. `by_row_id` takes a row id, never an entry id (Q-TRK-1). The live lists
/// (`watch_all`, `watch_by_entry`) are methods of the concrete `kuro-data` repository: a `Subscription` needs a tokio
/// runtime, and this trait is also implemented by in-memory fakes in tests.
pub trait TrackPort: Send + Sync {
    fn by_row_id(&self, row_id: Id) -> Result<Option<Track>, DomainError>;
    fn by_entry(&self, entry_id: Id) -> Result<Vec<Track>, DomainError>;
    /// Every binding of the kind.
    fn all(&self, kind: Kind) -> Result<Vec<Track>, DomainError>;
    /// Upserts by (entry, tracker): the row id stays the same on a second write (Q-TRK-2).
    fn insert(&self, track: &Track) -> Result<(), DomainError>;
    /// One transaction; a repeated pair leaves one row with the last values (TC-TRK-06).
    fn insert_all(&self, tracks: &[Track]) -> Result<(), DomainError>;
    /// Removes the binding of `entry_id` to `tracker_id`. Zero rows affected is `Ok` (TC-TRK-03).
    fn delete(&self, entry_id: Id, tracker_id: Id) -> Result<(), DomainError>;
}

impl TrackPort for TrackRepo {
    fn by_row_id(&self, row_id: Id) -> Result<Option<Track>, DomainError> {
        Ok(self.by_id(row_id)?)
    }
    fn by_entry(&self, entry_id: Id) -> Result<Vec<Track>, DomainError> {
        Ok(TrackRepo::by_entry(self, entry_id)?)
    }
    fn all(&self, kind: Kind) -> Result<Vec<Track>, DomainError> {
        Ok(TrackRepo::all(self, kind)?)
    }
    fn insert(&self, track: &Track) -> Result<(), DomainError> {
        Ok(TrackRepo::insert(self, track).map(drop)?)
    }
    fn insert_all(&self, tracks: &[Track]) -> Result<(), DomainError> {
        Ok(TrackRepo::insert_all(self, tracks)?)
    }
    fn delete(&self, entry_id: Id, tracker_id: Id) -> Result<(), DomainError> {
        Ok(TrackRepo::delete(self, entry_id, tracker_id)?)
    }
}
