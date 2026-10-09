//! Legacy return shapes for the interactor layer (T-05a-028, `p1-behavior.md` section 0).
//!
//! Kotlin's update and reset methods return `Boolean`, and its `remove` methods return nothing. Both
//! swallow the database error after logging it, so these wrappers do the same. Every other repository
//! method returns its `DataResult` unchanged.

use crate::DataResult;
use crate::anime::{AnimeRepo, AnimeUpdate};
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

/// `MangaRepo::update` as Kotlin's `Boolean`: `false` when the write failed.
pub fn update_manga(repo: &MangaRepo, u: &MangaUpdate) -> bool {
    bool_or_log("manga.update", repo.update(u))
}

/// `MangaRepo::update_all` as Kotlin's `Boolean`: `false` when the transaction rolled back.
pub fn update_all_manga(repo: &MangaRepo, updates: &[MangaUpdate]) -> bool {
    bool_or_log("manga.update_all", repo.update_all(updates))
}

/// `MangaRepo::reset_viewer_flags` as Kotlin's `Boolean`.
pub fn reset_manga_viewer_flags(repo: &MangaRepo) -> bool {
    bool_or_log("manga.reset_viewer_flags", repo.reset_viewer_flags())
}

/// `AnimeRepo::update` as Kotlin's `Boolean`.
pub fn update_anime(repo: &AnimeRepo, u: &AnimeUpdate) -> bool {
    bool_or_log("anime.update", repo.update(u))
}

/// `AnimeRepo::update_all` as Kotlin's `Boolean`.
pub fn update_all_anime(repo: &AnimeRepo, updates: &[AnimeUpdate]) -> bool {
    bool_or_log("anime.update_all", repo.update_all(updates))
}

/// `AnimeRepo::reset_viewer_flags` as Kotlin's `Boolean`.
pub fn reset_anime_viewer_flags(repo: &AnimeRepo) -> bool {
    bool_or_log("anime.reset_viewer_flags", repo.reset_viewer_flags())
}

/// `AnimeRepo::remove_parent_ids` as Kotlin's unit return: the error is logged, not returned.
pub fn remove_anime_parent_ids(repo: &AnimeRepo, ids: &[i64]) {
    unit_or_log("anime.remove_parent_ids", repo.remove_parent_ids(ids))
}
