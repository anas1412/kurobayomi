//! The player header's menu: the audio and subtitle tracks of the loaded file, read from mpv each time the menu
//! opens. A row click selects the track (rows go through `widgets::row_activate`).

use std::rc::Rc;

use gtk::prelude::*;
use kuro_player::mpv::MpvHandle;
use kuro_player::tracks::{TRACK_NONE, TrackProps, VideoTrack, load_track_lists, track_id_from_value, track_label};

use super::Shared;

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
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    crate::ui::margins(&column, 6);
    let popover = gtk::Popover::builder().child(&column).build();
    {
        let (shared, column) = (Rc::clone(shared), column.clone());
        popover.connect_show(move |popover| fill(&column, popover, &shared));
    }
    let menu = gtk::MenuButton::builder().icon_name("view-more-symbolic").popover(&popover).tooltip_text("Tracks").build();
    menu.update_property(&[gtk::accessible::Property::Label("Tracks")]);
    menu
}

fn fill(column: &gtk::Box, popover: &gtk::Popover, shared: &Shared) {
    while let Some(child) = column.first_child() {
        column.remove(&child);
    }
    let (lists, aid, sid) = {
        let player = shared.borrow();
        let mpv = player.playback.mpv();
        let current = |name: &str| track_id_from_value(mpv.get_string(name).as_deref());
        (load_track_lists(&MpvTracks(mpv)), current("aid"), current("sid"))
    };
    let mut subtitles = vec![VideoTrack { id: TRACK_NONE, name: "Off".to_owned(), language: None }];
    subtitles.extend(lists.subtitles);
    for (heading, property, tracks, selected) in [("Audio", "aid", lists.audio, aid), ("Subtitles", "sid", subtitles, sid)] {
        column.append(&gtk::Label::builder().label(heading).xalign(0.0).css_classes(["heading"]).build());
        let list = crate::ui::boxed_list();
        for track in tracks {
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.set_opacity(if track.id == selected { 1.0 } else { 0.0 });
            let row = crate::ui::row(&track_label(track.id, &track.name, track.language.as_deref()), None, Some(check.upcast()));
            row.set_activatable(true);
            let (shared, popover, id) = (Rc::clone(shared), popover.clone(), track.id);
            crate::widgets::row_activate::on_activate(&row, move |_| {
                popover.popdown();
                if let Err(e) = shared.borrow().playback.mpv().set_track(property, id) {
                    tracing::warn!(error = %e, property, "player: the track was not selected");
                }
            });
            list.append(&row);
        }
        column.append(&list);
    }
}
