//! The app side of the tracking flows (tasks T-38b-031 to T-38b-043 and T-39b-038 to T-39b-047). The flows themselves are
//! `kuro_track`; this file runs them through the tracking context the services built ([`tracker_host::ctx`]), on the app
//! runtime and detached, so a write survives the sheet or the reader closing. Without the services nothing runs and the
//! calls return `None`, which is what a user with no logged-in tracker gets.
//!
//! Callers: the reader calls [`chapter_read`] and the player calls [`episode_seen`] when a chapter or episode completes
//! (the reader's `ReaderExtras::track_update` sink); the track sheet and the tracker search call the rest.

use std::sync::Arc;

use kuro_track::bind::{spawn_register_anime, spawn_register_manga};
use kuro_track::ctx::TrackCtx;
use kuro_track::error::TrackError;
use kuro_track::messages::MsgKey;
use kuro_track::model::search::{AnimeTrackSearch, MangaTrackSearch};
use kuro_track::model::{AnimeTrack, MangaTrack, TrackerId};
use kuro_track::progress::{spawn_track_chapter, spawn_track_episode};
use kuro_track::remove::{spawn_remove_anime, spawn_remove_manga};
use kuro_track::write::{AnimeEdit, MangaEdit, spawn_anime_edit, spawn_manga_edit};
use tokio::task::JoinHandle;

use crate::settings_ui::tracker_host;

/// Runs `launch` with the tracking context and the app runtime entered, so the detached tasks it spawns find the runtime
/// from the GTK thread. `None` when the services are not built.
fn start<T>(launch: impl FnOnce(Arc<TrackCtx>) -> T) -> Option<T> {
    let ctx = tracker_host::ctx()?;
    let handle = crate::shell::host::Shell::get()?.runtime().clone();
    let _entered = handle.enter();
    Some(launch(ctx))
}

/// A chapter was read: its number goes to every logged-in manga tracker behind it. The flow checks the auto-update switch
/// itself, and the reader checks incognito before it calls this.
pub fn chapter_read(manga_id: i64, chapter: f64) {
    start(|ctx| spawn_track_chapter(ctx, manga_id, chapter));
}

/// An episode was seen: its number goes to every logged-in anime tracker behind it, as [`chapter_read`] does for manga.
pub fn episode_seen(anime_id: i64, episode: f64) {
    start(|ctx| spawn_track_episode(ctx, anime_id, episode));
}

/// Binds an anime entry to the search result `result` on `tracker`. The handle resolves to the stored row.
pub fn register_anime(entry_id: i64, result: AnimeTrackSearch, tracker: TrackerId, has_seen: bool) -> Option<JoinHandle<Result<AnimeTrack, TrackError>>> {
    start(|ctx| spawn_register_anime(ctx, entry_id, result, tracker, has_seen))
}

/// Binds a manga entry to the search result `result` on `tracker`. The handle resolves to the stored row.
pub fn register_manga(entry_id: i64, result: MangaTrackSearch, tracker: TrackerId, has_read: bool) -> Option<JoinHandle<Result<MangaTrack, TrackError>>> {
    start(|ctx| spawn_register_manga(ctx, entry_id, result, tracker, has_read))
}

/// One remote edit of an anime binding (status, progress, score, dates, private), written detached.
pub fn edit_anime(entry_id: i64, tracker: TrackerId, edit: AnimeEdit) -> Option<JoinHandle<Result<AnimeTrack, TrackError>>> {
    start(|ctx| spawn_anime_edit(ctx, entry_id, tracker, edit))
}

/// One remote edit of a manga binding, written detached.
pub fn edit_manga(entry_id: i64, tracker: TrackerId, edit: MangaEdit) -> Option<JoinHandle<Result<MangaTrack, TrackError>>> {
    start(|ctx| spawn_manga_edit(ctx, entry_id, tracker, edit))
}

/// Removes an anime binding, and the remote entry too when `also_remote` is set. The handle resolves to the message to
/// show when it failed.
pub fn remove_anime(track: AnimeTrack, also_remote: bool) -> Option<JoinHandle<Result<(), MsgKey>>> {
    start(|ctx| spawn_remove_anime(ctx, track, also_remote))
}

/// Removes a manga binding, and the remote entry too when `also_remote` is set.
pub fn remove_manga(track: MangaTrack, also_remote: bool) -> Option<JoinHandle<Result<(), MsgKey>>> {
    start(|ctx| spawn_remove_manga(ctx, track, also_remote))
}

/// Searches AniList for anime titles (the tracker search screen).
pub fn search_anime(query: String) -> Option<JoinHandle<Result<Vec<AnimeTrackSearch>, TrackError>>> {
    start(|ctx| tokio::spawn(anilist_search_anime(ctx, query)))
}

/// Searches AniList for manga titles (the tracker search screen).
pub fn search_manga(query: String) -> Option<JoinHandle<Result<Vec<MangaTrackSearch>, TrackError>>> {
    start(|ctx| tokio::spawn(anilist_search_manga(ctx, query)))
}

/// Refreshes every logged-in anime binding of the entry from its service (the sheet's refresh). The handle resolves to the
/// failures, which the caller shows; a failed refresh keeps the local row as it was.
pub fn refresh_anime(entry_id: i64) -> Option<JoinHandle<Result<Vec<(TrackerId, TrackError)>, TrackError>>> {
    start(|ctx| tokio::spawn(async move { kuro_track::sync::refresh_anime(&ctx, entry_id).await }))
}

/// Manga form of [`refresh_anime`].
pub fn refresh_manga(manga_id: i64) -> Option<JoinHandle<Result<Vec<(TrackerId, TrackError)>, TrackError>>> {
    start(|ctx| tokio::spawn(async move { kuro_track::sync::refresh_manga(&ctx, manga_id).await }))
}

async fn anilist_search_anime(ctx: Arc<TrackCtx>, query: String) -> Result<Vec<AnimeTrackSearch>, TrackError> {
    let tracker = ctx.trackers.get(TrackerId::ANILIST).ok_or(TrackError::UnknownTracker(TrackerId::ANILIST.0))?;
    let anime = tracker.as_anime().ok_or_else(|| TrackError::Message("AniList has no anime list".into()))?;
    anime.search(&query).await
}

async fn anilist_search_manga(ctx: Arc<TrackCtx>, query: String) -> Result<Vec<MangaTrackSearch>, TrackError> {
    let tracker = ctx.trackers.get(TrackerId::ANILIST).ok_or(TrackError::UnknownTracker(TrackerId::ANILIST.0))?;
    let manga = tracker.as_manga().ok_or_else(|| TrackError::Message("AniList has no manga list".into()))?;
    manga.search(&query).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_the_services_every_call_is_a_no_op() {
        // the services are built on the GTK thread at run time: a unit test has none, so nothing starts
        assert!(tracker_host::ctx().is_none());
        chapter_read(1, 2.0);
        episode_seen(1, 2.0);
        assert!(search_anime("x".into()).is_none());
        assert!(search_manga("x".into()).is_none());
        assert!(refresh_anime(1).is_none() && refresh_manga(1).is_none());
    }
}
