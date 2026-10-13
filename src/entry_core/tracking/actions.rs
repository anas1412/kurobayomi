//! The app side of the tracking flows (tasks T-38b-031 to T-38b-043 and T-39b-038 to T-39b-047). The flows themselves are
//! `kuro_track`; this file runs them through the tracking context the services built ([`tracker_host::ctx`]), on the app
//! runtime and detached, so a write survives the sheet or the reader closing. Without the services nothing runs and the
//! calls return `None`, which is what a user with no logged-in tracker gets.
//!
//! Callers: the reader calls [`chapter_read`] and the player calls [`episode_seen`] when a chapter or episode completes
//! (the reader's `ReaderExtras::track_update` sink); the track sheet and the tracker search call the rest.

use std::sync::Arc;

use kuro_track::ctx::TrackCtx;
use kuro_track::error::TrackError;
use kuro_track::messages::MsgKey;
use kuro_track::model::{AnimeTrack, MangaTrack, TrackerId};
use kuro_track::progress::{TrackOutcome, spawn_track_chapter, spawn_track_episode};
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

/// What each tracker did with a progress update.
pub type Outcomes = JoinHandle<Vec<(TrackerId, TrackOutcome)>>;

/// A chapter was read: its number goes to every logged-in manga tracker behind it. The caller checks the auto-update
/// switch and incognito before it calls this (`progress::item_finished`, `progress::marked`).
pub fn chapter_read(manga_id: i64, chapter: f64) -> Option<Outcomes> {
    start(|ctx| spawn_track_chapter(ctx, manga_id, chapter))
}

/// An episode was seen: its number goes to every logged-in anime tracker behind it, as [`chapter_read`] does for manga.
pub fn episode_seen(anime_id: i64, episode: f64) -> Option<Outcomes> {
    start(|ctx| spawn_track_episode(ctx, anime_id, episode))
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
