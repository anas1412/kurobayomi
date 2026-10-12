//! The quality and server menu of an online episode (docs/design/player-controls.md). When the menu opens, the episode's
//! servers and their videos load on the source's runtime (a spinner until they do, then the list is kept for the
//! episode). Choosing a video resolves its stream and reloads it at the playing position, with that video's headers,
//! subtitles and audio. The last choice is kept for the session: the next episode plays the same server and title when
//! its list has them, else the source's best video. A local episode has no menu. The video's right-click menu lists the
//! same choices through [`menu_sections`], over the same loaded servers ([`Servers`]).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gtk::glib;
use gtk::prelude::*;
use kuro_data::entry::EntryRepo;
use kuro_data::episode::Episode;
use kuro_data::item::ItemRepo;
use kuro_domain::source::remote_update::to_s_episode;
use kuro_i18n::{MsgKey, tr};
use kuro_source::SourceError;
use kuro_source::anime::{
    AnimeSource, Hoster, HosterState, NO_HOSTER_LIST, SEpisode, Video, best_video, http_hosters, load_hoster, resolve_once,
};

use super::super::{Media, Shared, source, stored_text};
use super::{Choice, clear, menu, section};

/// A server and a video title: the choice the session keeps.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    hoster: String,
    title: String,
}

/// The last choice, which is also the one playing. A static lock, because [`choose`] runs on the source's runtime,
/// not on the GTK thread.
static LAST: Mutex<Option<Pick>> = Mutex::new(None);

fn last() -> Option<Pick> {
    LAST.lock().ok().and_then(|guard| (*guard).clone())
}

fn set_last(pick: Option<Pick>) {
    if let Ok(mut last) = LAST.lock() {
        *last = pick;
    }
}

/// The servers of an episode that the open menu lists, kept for the episode they belong to.
struct Loaded {
    episode_id: i64,
    states: Vec<HosterState>,
}

/// The servers of the playing episode, loaded once per episode and shared by the quality button and the video's
/// right-click menu. Cloning shares them.
#[derive(Clone, Default)]
pub struct Servers(Rc<ServersInner>);

#[derive(Default)]
struct ServersInner {
    loaded: RefCell<Option<Loaded>>,
    /// The episode whose servers are loading: asking again meanwhile does not ask the source twice.
    pending: Cell<Option<i64>>,
}

impl Servers {
    /// The loaded servers of `episode_id`, or `None` while they load. The first ask starts the load and `done` runs with
    /// its answer; an ask while it runs starts nothing and its `done` is dropped.
    fn get_or_load(&self, episode_id: i64, source: Arc<dyn AnimeSource>, episode: SEpisode, done: impl FnOnce(Result<Vec<HosterState>, String>) + 'static) -> Option<Vec<HosterState>> {
        let cached = self.0.loaded.borrow().as_ref().filter(|loaded| loaded.episode_id == episode_id).map(|loaded| loaded.states.clone());
        if cached.is_some() {
            return cached;
        }
        if self.0.pending.get() == Some(episode_id) {
            return None;
        }
        self.0.pending.set(Some(episode_id));
        let this = self.clone();
        load(source, episode, move |answer| {
            this.0.pending.set(None);
            if let Ok(states) = &answer {
                *this.0.loaded.borrow_mut() = Some(Loaded { episode_id, states: states.clone() });
            }
            done(answer);
        });
        None
    }
}

/// The quality and server button over `servers`. Its list loads the first time it opens for an episode.
pub fn button(shared: &Shared, servers: &Servers) -> gtk::MenuButton {
    let button = menu("Quality and server", {
        let (shared, servers) = (Rc::clone(shared), servers.clone());
        move |column, popover| fill(column, popover, &shared, &servers)
    });
    button.set_icon_name("video-display-symbolic");
    button
}

/// The choices of the quality menu for the video's right-click menu: one list per server, with its heading. `None` for
/// an episode that is not online; an empty list while the servers load (the first ask starts the load, and a failure
/// shows its toast).
pub fn menu_sections(shared: &Shared, servers: &Servers) -> Option<Vec<(Option<String>, Vec<Choice>)>> {
    let episode_id = shared.borrow().episode_id;
    let (source, episode) = online_episode(episode_id)?;
    let states = servers.get_or_load(episode_id, Arc::clone(&source), episode, |answer| {
        if let Err(text) = answer {
            crate::toast::notify(text);
        }
    });
    Some(states.map_or_else(Vec::new, |states| sections(shared, &source, &states)))
}

/// Whether the playing episode is online, which is the only time the menu has servers to choose from.
pub fn is_online(episode_id: i64) -> bool {
    online_episode(episode_id).is_some()
}

/// The video an episode plays (the source's stream choice; see `best_stream` in `source.rs`). The last choice is
/// used when this episode's servers have the same server and title; the choice made here is then kept for the session.
pub async fn choose(source: &dyn AnimeSource, hosters: &[Hoster]) -> Result<Option<Video>, SourceError> {
    if let Some(pick) = last() {
        if let Some(video) = remembered(source, hosters, &pick).await {
            if let Some(resolved) = resolve_once(source, video).await? {
                set_last(Some(pick));
                return Ok(Some(resolved));
            }
        }
    }
    let best = best_video(source, hosters).await?;
    set_last(best.as_ref().and_then(|video| identify(hosters, &video.video_title)));
    Ok(best)
}

/// The video of `pick` among its server's videos. A lazy server's list is fetched here.
async fn remembered(source: &dyn AnimeSource, hosters: &[Hoster], pick: &Pick) -> Option<Video> {
    for hoster in hosters.iter().filter(|h| h.hoster_name == pick.hoster) {
        let videos = match &hoster.video_list {
            Some(videos) => videos.clone(),
            None => match source.videos_for_hoster(hoster).await {
                Ok(videos) => videos,
                Err(_) => continue,
            },
        };
        if let Some(video) = videos.into_iter().find(|v| v.video_title == pick.title) {
            return Some(video);
        }
    }
    None
}

/// The server that lists a video called `title`. Only servers whose list is already known can say.
fn identify(hosters: &[Hoster], title: &str) -> Option<Pick> {
    hosters
        .iter()
        .find(|h| h.video_list.as_ref().is_some_and(|videos| videos.iter().any(|v| v.video_title == title)))
        .map(|h| Pick { hoster: h.hoster_name.clone(), title: title.to_owned() })
}

/// The source and the episode row of `episode_id`. `None` for a local episode, an unknown row or a source that is not
/// installed.
fn online_episode(episode_id: i64) -> Option<(Arc<dyn AnimeSource>, SEpisode)> {
    if !crate::data::is_installed() {
        return None;
    }
    let db = &crate::data::app().db;
    let episode = ItemRepo::new(Arc::clone(db)).by_id(episode_id).ok()??;
    let anime = EntryRepo::new(Arc::clone(db)).by_id(episode.entry_id).ok()?;
    let source = crate::screens::anime_sources().into_iter().find(|s| s.id() == anime.source)?;
    Some((source, to_s_episode(&Episode::from(episode))))
}

/// Reopening the menu while the servers load shows the spinner again but does not ask the source twice.
fn fill(column: &gtk::Box, popover: &gtk::Popover, shared: &Shared, servers: &Servers) {
    let episode_id = shared.borrow().episode_id;
    let Some((source, episode)) = online_episode(episode_id) else {
        return;
    };
    let (shown_column, shown_popover, shown_shared, loaded_source) = (column.clone(), popover.clone(), Rc::clone(shared), Arc::clone(&source));
    let states = servers.get_or_load(episode_id, Arc::clone(&source), episode, move |answer| match answer {
        Ok(states) => {
            if shown_popover.is_visible() {
                clear(&shown_column);
                list(&shown_column, &shown_popover, &shown_shared, &loaded_source, &states);
            }
        }
        Err(text) => {
            clear(&shown_column);
            crate::toast::notify(text);
        }
    });
    match states {
        Some(states) => list(column, popover, shared, &source, &states),
        // a load that failed at once has already shown its toast
        None if servers.0.pending.get() == Some(episode_id) => column.append(&gtk::Spinner::builder().spinning(true).halign(gtk::Align::Center).margin_top(12).margin_bottom(12).build()),
        None => {}
    }
}

/// One heading per server (none for the list of a source without servers), then its videos. A server that failed to
/// load shows one row that cannot be chosen; a server with no videos is left out.
fn list(column: &gtk::Box, popover: &gtk::Popover, shared: &Shared, source: &Arc<dyn AnimeSource>, states: &[HosterState]) {
    for (heading, choices) in sections(shared, source, states) {
        section(column, heading.as_deref(), choices, popover);
    }
}

/// The lists `list` shows: each server's heading and its rows.
fn sections(shared: &Shared, source: &Arc<dyn AnimeSource>, states: &[HosterState]) -> Vec<(Option<String>, Vec<Choice>)> {
    let last = last();
    states
        .iter()
        .filter_map(|state| match state {
            HosterState::Ready { hoster_name, videos } if !videos.is_empty() => {
                let choices = videos.iter().map(|entry| choice(shared, source, hoster_name, &entry.video, last.as_ref())).collect();
                Some((heading(hoster_name).map(str::to_owned), choices))
            }
            HosterState::Error { hoster_name } => {
                let failed = Choice { title: "Could not load".to_owned(), subtitle: None, current: false, enabled: false, pick: Box::new(|| {}) };
                Some((heading(hoster_name).map(str::to_owned), vec![failed]))
            }
            _ => None,
        })
        .collect()
}

fn heading(hoster_name: &str) -> Option<&str> {
    (hoster_name != NO_HOSTER_LIST).then_some(hoster_name)
}

fn choice(shared: &Shared, source: &Arc<dyn AnimeSource>, hoster: &str, video: &Video, last: Option<&Pick>) -> Choice {
    let pick = Pick { hoster: hoster.to_owned(), title: video.video_title.clone() };
    let current = last == Some(&pick);
    let (shared, source, video) = (Rc::clone(shared), Arc::clone(source), video.clone());
    Choice {
        title: video.video_title.clone(),
        subtitle: None,
        current,
        enabled: true,
        pick: Box::new(move || switch(&shared, Arc::clone(&source), pick.clone(), video.clone())),
    }
}

/// The servers of an episode with their videos, every one loaded (a lazy server too). Runs on the source's runtime and
/// calls `done` on the GTK thread.
fn load(source: Arc<dyn AnimeSource>, episode: SEpisode, done: impl FnOnce(Result<Vec<HosterState>, String>) + 'static) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        done(Err(tr(MsgKey::player_hoster_failed)));
        return;
    };
    let (tx, rx) = async_channel::bounded(1);
    handle.spawn(async move {
        // the spinner of the menu never stays: after 30 seconds the toast says the source is not answering
        let answer = crate::browse::errors::settle(crate::browse::errors::LOAD_TIMEOUT, source.name(), all_states(source.as_ref(), &episode)).await;
        let _ = tx.send(answer).await;
    });
    glib::spawn_future_local(async move {
        match rx.recv().await {
            Ok(answer) => done(answer),
            // a closed channel means the source task panicked
            Err(_) => done(Err(tr(MsgKey::no_hosters))),
        }
    });
}

async fn all_states(source: &dyn AnimeSource, episode: &SEpisode) -> Result<Vec<HosterState>, String> {
    let hosters = crate::browse::errors::plain(source.name(), http_hosters(source, episode)).await?;
    let mut states = Vec::with_capacity(hosters.len());
    for hoster in &hosters {
        let state = load_hoster(source, hoster, true)
            .await
            .unwrap_or_else(|_| HosterState::Error { hoster_name: hoster.hoster_name.clone() });
        states.push(state);
    }
    Ok(states)
}

/// Plays `video` of `pick` in the open page: its stream resolves in the background (a spinner shows), then the page
/// reloads at the playing position. A second choice waits for the first, as a previous or next episode does.
fn switch(shared: &Shared, source: Arc<dyn AnimeSource>, pick: Pick, video: Video) {
    let title = {
        let mut player = shared.borrow_mut();
        if player.resolving {
            return;
        }
        player.resolving = true;
        let title = player.media.title.clone();
        title
    };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        shared.borrow_mut().resolving = false;
        crate::toast::notify(tr(MsgKey::player_hoster_failed));
        return;
    };
    let (tx, rx) = async_channel::bounded(1);
    handle.spawn(async move {
        let _ = tx.send(crate::browse::errors::plain(source.name(), resolve_once(source.as_ref(), video)).await).await;
    });
    let shared = Rc::clone(shared);
    glib::spawn_future_local(async move {
        // a closed channel means the source task panicked
        let answer = rx.recv().await.unwrap_or_else(|_| Err(tr(MsgKey::video_list_empty_error)));
        shared.borrow_mut().resolving = false;
        match answer {
            Ok(Some(resolved)) => {
                set_last(Some(pick));
                let preferred = stored_text("pref_subtitle_lang");
                let preferred_audio = stored_text("pref_audio_lang");
                reload(&shared, source::media_of(resolved, title, &preferred, &preferred_audio));
            }
            Ok(None) => crate::toast::notify(tr(MsgKey::video_list_empty_error)),
            Err(text) => crate::toast::notify(text),
        }
    });
}

/// Loads `media` at the playing position and keeps the pause state. Its side tracks are added when mpv has loaded it.
fn reload(shared: &Shared, media: Media) {
    let mut player = shared.borrow_mut();
    if player.closed {
        return;
    }
    let start = player.playback.position_ms().unwrap_or(0);
    let paused = player.playback.is_paused();
    player.media = media;
    if let Err(e) = player.playback.load(&player.media.target, player.media.headers.as_deref(), start) {
        tracing::error!(error = %e, "player: the chosen quality could not load");
        crate::toast::notify(tr(MsgKey::player_hoster_failed));
        return;
    }
    if let Err(e) = player.playback.mpv().set_property_flag("pause", paused) {
        tracing::warn!(error = %e, "player: the pause state was not restored");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hoster(name: &str, titles: &[&str]) -> Hoster {
        let videos = titles.iter().map(|t| Video::new(format!("https://cdn.test/{t}"), *t)).collect();
        Hoster { hoster_name: name.into(), video_list: Some(videos), ..Hoster::default() }
    }

    #[test]
    fn a_title_is_identified_by_the_server_that_lists_it() {
        let hosters = vec![hoster("Alpha", &["480p"]), hoster("Beta", &["1080p", "720p"])];
        assert_eq!(identify(&hosters, "720p"), Some(Pick { hoster: "Beta".into(), title: "720p".into() }));
        assert_eq!(identify(&hosters, "2160p"), None, "a title no server lists is not identified");
    }

    #[test]
    fn a_server_with_a_lazy_list_cannot_be_identified_yet() {
        let lazy = Hoster { hoster_name: "Lazy".into(), lazy: true, ..Hoster::default() };
        assert_eq!(identify(&[lazy], "1080p"), None);
    }

    #[test]
    fn the_legacy_list_has_no_heading() {
        assert_eq!(heading(NO_HOSTER_LIST), None);
        assert_eq!(heading("Alpha"), Some("Alpha"));
    }
}
