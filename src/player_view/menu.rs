//! The player header's audio and subtitle menu: the tracks of the loaded file, read from mpv each time the menu opens.
//! Both lists start with Off (audio Off comes from `load_track_lists`, subtitle Off is added here). The current track
//! carries the check mark. A row click selects the track. The lists use the shared popover helper in `sheets`.

use std::rc::Rc;

use gtk::prelude::*;
use kuro_player::mpv::MpvHandle;
use kuro_player::tracks::{TRACK_NONE, TrackProps, VideoTrack, load_track_lists, track_id_from_value, track_label};

use super::Shared;
use super::sheets::{self, Choice};

/// mpv's `track-list/*` properties as the track lists read them.
pub struct MpvTracks<'a>(pub &'a MpvHandle);

impl TrackProps for MpvTracks<'_> {
    fn count(&self) -> Option<i64> {
        Some(self.0.get_i64("track-list/count").unwrap_or(0))
    }
    fn kind(&self, index: i64) -> Option<String> {
        self.0.get_string(&format!("track-list/{index}/type"))
    }
    fn lang(&self, index: i64) -> Option<String> {
        self.0.get_string(&format!("track-list/{index}/lang"))
    }
    fn title(&self, index: i64) -> Option<String> {
        self.0.get_string(&format!("track-list/{index}/title"))
    }
    fn id(&self, index: i64) -> Option<i64> {
        self.0.get_i64(&format!("track-list/{index}/id"))
    }
}

/// The menu button. Its list is rebuilt whenever it opens, so tracks added after the load show up.
pub fn tracks_menu(shared: &Shared) -> gtk::MenuButton {
    let shared = Rc::clone(shared);
    let menu = sheets::menu("Tracks", move |column, popover| fill(column, popover, &shared));
    menu.set_icon_name("view-more-symbolic");
    menu
}

fn fill(column: &gtk::Box, popover: &gtk::Popover, shared: &Shared) {
    for (heading, choices) in track_sections(shared) {
        sheets::section(column, Some(heading), choices, popover);
    }
}

/// The two lists of the menu, read from mpv now: Audio, then Subtitles, each with the playing track marked. The video's
/// right-click menu lists the same.
pub fn track_sections(shared: &Shared) -> Vec<(&'static str, Vec<Choice>)> {
    let (lists, aid, sid) = {
        let player = shared.borrow();
        let mpv = player.playback.mpv();
        let current = |name: &str| track_id_from_value(mpv.get_string(name).as_deref());
        (load_track_lists(&MpvTracks(mpv)), current("aid"), current("sid"))
    };
    let mut subtitles = vec![VideoTrack { id: TRACK_NONE, name: "Off".to_owned(), language: None }];
    subtitles.extend(lists.subtitles);
    [("Audio", "aid", lists.audio, aid), ("Subtitles", "sid", subtitles, sid)]
        .into_iter()
        .map(|(heading, property, tracks, selected)| (heading, tracks.into_iter().map(|track| choice(shared, property, track, selected)).collect()))
        .collect()
}

/// One track row. Choosing it selects the track in mpv under `property` (`aid` or `sid`).
fn choice(shared: &Shared, property: &'static str, track: VideoTrack, selected: i64) -> Choice {
    let title = track_label(track.id, &track.name, track.language.as_deref());
    let (shared, id) = (Rc::clone(shared), track.id);
    Choice {
        title,
        subtitle: None,
        current: id == selected,
        enabled: true,
        pick: Box::new(move || {
            if let Err(e) = shared.borrow().playback.mpv().set_track(property, id) {
                tracing::warn!(error = %e, property, "player: the track was not selected");
            }
        }),
    }
}
