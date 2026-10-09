//! Player settings: the root with its nine rows (T-48a-010), and the pages of the rows built so far.
pub mod controls;
pub mod files;
pub mod gestures;
pub mod intro;
pub mod root;
pub mod scripts;
pub mod seeking;

use std::rc::Rc;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::rows::{group, text};

/// Below this window width the root list is a page with a back button; at or above it, a sidebar and a page.
pub const SIDEBAR_MIN_WIDTH: i32 = 720;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Sidebar,
    Narrow,
}

pub fn layout_for(width: i32) -> Layout {
    if width >= SIDEBAR_MIN_WIDTH { Layout::Sidebar } else { Layout::Narrow }
}

/// "Player" when opened from the main settings, "Player settings" otherwise.
#[allow(dead_code)] // the page title comes from the spec screen, which has one title
pub fn title(main_settings: bool) -> &'static str {
    if main_settings { "Player" } else { "Player settings" }
}

/// One root row: the sub-screen it opens is `target`, a settings screen id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootRow {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub icon: &'static str,
    pub target: &'static str,
}

/// The nine rows in order (`PlayerSettingsMainScreen.kt`).
pub const ROOT_ROWS: [RootRow; 9] = [
    RootRow { title: "Internal player", subtitle: "Progress, controls, orientation", icon: "media-playback-start-symbolic", target: "settings_player_internal" },
    RootRow { title: "Gestures", subtitle: "Seeking, double tap, media controls", icon: "input-touchpad-symbolic", target: "settings_player_gestures" },
    RootRow { title: "Decoder", subtitle: "Hardware decoding, pixel format, debanding", icon: "cpu-symbolic", target: "settings_player_decoder" },
    RootRow { title: "Subtitles", subtitle: "Preferred languages, whitelist, blacklist", icon: "media-view-subtitles-symbolic", target: "settings_player_subtitle" },
    RootRow { title: "Audio", subtitle: "Preferred languages, pitch correction, audio channels", icon: "audio-x-generic-symbolic", target: "settings_player_audio" },
    RootRow { title: "Custom buttons", subtitle: "Execute lua code with custom buttons", icon: "utilities-terminal-symbolic", target: "settings_player_custom_buttons" },
    RootRow { title: "Script editor", subtitle: "Edit and create new scripts and script options", icon: "document-edit-symbolic", target: "settings_player_editor" },
    RootRow { title: "Torrent", subtitle: "TorrServer options", icon: "network-transmit-receive-symbolic", target: "settings_player_torrents" },
    RootRow { title: "Advanced", subtitle: "User files, mpv.conf, input.conf", icon: "text-x-script-symbolic", target: "settings_player_advanced" },
];

/// The root list as a preference tree: one row per entry; a tap runs `open` with the row's `target`.
pub fn root_rows(open: Rc<dyn Fn(&'static str)>) -> Vec<Preference> {
    let rows: Vec<PreferenceItem> = ROOT_ROWS
        .iter()
        .map(|row| {
            let mut item = text(row.title, Some(row.subtitle), true);
            if let PreferenceItem::Text { common, on_click } = &mut item {
                common.icon = Some(row.icon);
                let (open, target) = (Rc::clone(&open), row.target);
                *on_click = Some(Rc::new(move || open(target)));
            }
            item
        })
        .collect();
    vec![group("", rows)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_rows_with_the_spec_targets_in_order() {
        let targets: Vec<&str> = ROOT_ROWS.iter().map(|r| r.target).collect();
        assert_eq!(ROOT_ROWS.len(), 9);
        assert_eq!(targets[0], "settings_player_internal");
        assert_eq!(targets[8], "settings_player_advanced");
        let tree = root_rows(Rc::new(|_| {}));
        let Preference::Group(g) = &tree[0] else { panic!("group") };
        assert_eq!(g.items.len(), 9);
    }

    #[test]
    fn tapping_a_row_opens_its_target() {
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let tree = root_rows(Rc::new(move |target| sink.borrow_mut().push(target)));
        let Preference::Group(g) = &tree[0] else { panic!("group") };
        for item in &g.items {
            let PreferenceItem::Text { on_click, .. } = item else { panic!("text row") };
            (on_click.as_ref().expect("a tap handler"))();
        }
        let targets: Vec<&str> = ROOT_ROWS.iter().map(|r| r.target).collect();
        assert_eq!(*seen.borrow(), targets);
    }

    #[test]
    fn every_root_target_is_a_spec_screen_and_the_icons_follow_the_spec_note() {
        crate::ui::with_spec(|spec| {
            for row in ROOT_ROWS {
                assert!(spec.screen(row.target).is_some(), "{}", row.target);
            }
        });
        let icons: Vec<&str> = ROOT_ROWS.iter().map(|r| r.icon).collect();
        assert_eq!(icons[0], "media-playback-start-symbolic", "play-circle");
        assert_eq!(icons[2], "cpu-symbolic", "memory chip");
        assert_eq!(icons[5], "utilities-terminal-symbolic", "terminal");
        assert_eq!(icons[6], "document-edit-symbolic", "edit-note");
    }

    #[test]
    fn breakpoint_is_720_px_inclusive() {
        assert_eq!(layout_for(720), Layout::Sidebar);
        assert_eq!(layout_for(719), Layout::Narrow);
    }

    #[test]
    fn title_depends_on_the_entry_point() {
        assert_eq!(title(true), "Player");
        assert_eq!(title(false), "Player settings");
    }
}
