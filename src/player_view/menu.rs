//! The player header's audio and subtitle menu: the tracks of the loaded file, read from mpv each time the menu opens.
//! Both lists start with Off (audio Off comes from `load_track_lists`, subtitle Off is added here). The current track
//! carries the check mark. A row click selects the track. The lists use the shared popover helper in `sheets`.
//!
//! Only five languages are listed (user, 2026-10-15): English, Arabic, French, Spanish and Japanese (the original
//! audio of most anime). A track with no language tag, and the track playing now, are always listed.

use std::rc::Rc;

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
pub fn tracks_menu(shared: &Shared, servers: &sheets::quality::Servers) -> gtk::MenuButton {
    let (shared, servers) = (Rc::clone(shared), servers.clone());
    let menu = sheets::menu("Tracks", move |column, popover| {
        // an online episode lists its servers first, then the tracks
        if sheets::quality::is_online(shared.borrow().episode_id) {
            sheets::quality::fill_servers(column, popover, &shared, &servers);
        }
        fill(column, popover, &shared);
    });
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
        .map(|(heading, property, tracks, selected)| {
            let shown = tracks.into_iter().filter(|track| track.id == selected || listed(track.language.as_deref(), &track.name));
            (heading, shown.map(|track| choice(shared, property, track, selected)).collect())
        })
        .collect()
}

/// The language tags the menus list, ISO 639-1 and both 639-2 forms: English, Arabic, French, Spanish, Japanese. A
/// regional tag ("es-419") counts by its first part.
const LISTED: [&str; 11] = ["en", "eng", "ar", "ara", "fr", "fre", "fra", "es", "spa", "ja", "jpn"];

/// Whether a track is one of the five languages (or names none): its tag, else the one a side track carries at the end
/// of its title ("Portuguese (por)"). The player also adds only these side tracks (`source::media_of`).
pub(super) fn listed(tag: Option<&str>, name: &str) -> bool {
    listed_language(language_of(tag, name).as_deref())
}

fn language_of(tag: Option<&str>, name: &str) -> Option<String> {
    tag.map(str::to_owned).filter(|l| !l.trim().is_empty()).or_else(|| {
        let inner = name.trim().strip_suffix(')')?.rsplit_once('(')?.1.trim();
        (2..=6).contains(&inner.len()).then(|| inner.to_owned())
    })
}

/// Whether a track of `language` is listed: one of the five languages, or no language tag at all.
fn listed_language(language: Option<&str>) -> bool {
    let Some(tag) = language.map(str::trim).filter(|t| !t.is_empty()) else { return true };
    let tag = tag.to_ascii_lowercase();
    let primary = tag.split(['-', '_']).next().unwrap_or(&tag);
    LISTED.contains(&primary)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_languages_and_untagged_tracks_are_listed() {
        for tag in ["eng", "en", "ara", "fre", "fra", "spa", "es-419", "jpn", "JA"] {
            assert!(listed_language(Some(tag)), "{tag}");
        }
        for tag in ["por", "rus", "ger", "it"] {
            assert!(!listed_language(Some(tag)), "{tag}");
        }
        assert!(listed_language(None) && listed_language(Some("")), "a track without a tag stays");
        assert_eq!(language_of(None, "Portuguese (por)").as_deref(), Some("por"));
        assert_eq!(language_of(None, "English"), None, "no tag: listed");
        assert_eq!(language_of(Some("jpn"), "x (por)").as_deref(), Some("jpn"), "mpv's tag first");
        assert!(!listed(None, "Portuguese (por)") && listed(None, "Arabic (ar)") && listed(None, "English"));
    }
}
