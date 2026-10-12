//! Legacy return shapes for the interactor layer (T-05a-028, `p1-behavior.md` section 0).
//!
//! Kotlin's update and reset methods return `Boolean`, and its `remove` methods return nothing. Both
//! swallow the database error after logging it, so these wrappers do the same. Every other repository
//! method returns its `DataResult` unchanged.

use kuro_platform::kind::Kind;

use crate::DataResult;
use crate::anime::{AnimeRepo, AnimeUpdate};
use crate::entry::{EntryRepo, EntryUpdate};
use crate::manga::{MangaRepo, MangaUpdate};

fn bool_or_log(operation: &str, result: DataResult<()>) -> bool {
    match result {
        Ok(()) => true,
        Err(e) => {
            tracing::error!(operation, error = %e, "database write failed");
            false
        }
    }
}

fn unit_or_log(operation: &str, result: DataResult<()>) {
    if let Err(e) = result {
        tracing::error!(operation, error = %e, "database write failed");
    }
}

/// `EntryRepo::update` as Kotlin's `Boolean`: `false` when the write failed.
pub fn update_entry(repo: &EntryRepo, u: &EntryUpdate) -> bool {
    bool_or_log("entry.update", repo.update(u))
}

/// `EntryRepo::update_all` as Kotlin's `Boolean`: `false` when the transaction rolled back.
pub fn update_all_entries(repo: &EntryRepo, updates: &[EntryUpdate]) -> bool {
    bool_or_log("entry.update_all", repo.update_all(updates))
}

/// `EntryRepo::reset_viewer_flags` as Kotlin's `Boolean`.
pub fn reset_viewer_flags(repo: &EntryRepo, kind: Kind) -> bool {
    bool_or_log("entry.reset_viewer_flags", repo.reset_viewer_flags(kind))
}

/// `EntryRepo::remove_parent_ids` as Kotlin's unit return: the error is logged, not returned.
pub fn remove_parent_ids(repo: &EntryRepo, ids: &[i64]) {
    unit_or_log("entry.remove_parent_ids", repo.remove_parent_ids(ids))
}

// ----- the old per-kind names; phase E deletes them -----

/// [`update_entry`] over a `MangaRepo`.
pub fn update_manga(repo: &MangaRepo, u: &MangaUpdate) -> bool {
    update_entry(&repo.entries, &u.into())
}

/// [`update_all_entries`] over a `MangaRepo`.
pub fn update_all_manga(repo: &MangaRepo, updates: &[MangaUpdate]) -> bool {
    update_all_entries(&repo.entries, &updates.iter().map(EntryUpdate::from).collect::<Vec<_>>())
}

/// [`reset_viewer_flags`] over a `MangaRepo`.
pub fn reset_manga_viewer_flags(repo: &MangaRepo) -> bool {
    reset_viewer_flags(&repo.entries, Kind::Manga)
}

/// [`update_entry`] over an `AnimeRepo`.
pub fn update_anime(repo: &AnimeRepo, u: &AnimeUpdate) -> bool {
    update_entry(&repo.entries, &u.into())
}

/// [`update_all_entries`] over an `AnimeRepo`.
pub fn update_all_anime(repo: &AnimeRepo, updates: &[AnimeUpdate]) -> bool {
    update_all_entries(&repo.entries, &updates.iter().map(EntryUpdate::from).collect::<Vec<_>>())
}

/// [`reset_viewer_flags`] over an `AnimeRepo`.
pub fn reset_anime_viewer_flags(repo: &AnimeRepo) -> bool {
    reset_viewer_flags(&repo.entries, Kind::Anime)
}

/// [`remove_parent_ids`] over an `AnimeRepo`.
pub fn remove_anime_parent_ids(repo: &AnimeRepo, ids: &[i64]) {
    remove_parent_ids(&repo.entries, ids)
}
