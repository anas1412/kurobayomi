//! Player settings: the root with its nine rows (T-48a-010), and the pages of the rows built so far.
pub mod controls;
pub mod gestures;
pub mod seeking;

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
    RootRow { title: "Internal player", subtitle: "Progress, controls, orientation", icon: "video-display-symbolic", target: "settings_player_internal" },
    RootRow { title: "Gestures", subtitle: "Seeking, double tap, media controls", icon: "input-touchpad-symbolic", target: "settings_player_gestures" },
    RootRow { title: "Decoder", subtitle: "Hardware decoding, pixel format, debanding", icon: "cpu-symbolic", target: "settings_player_decoder" },
    RootRow { title: "Subtitles", subtitle: "Preferred languages, whitelist, blacklist", icon: "media-view-subtitles-symbolic", target: "settings_player_subtitle" },
    RootRow { title: "Audio", subtitle: "Preferred languages, pitch correction, audio channels", icon: "audio-speakers-symbolic", target: "settings_player_audio" },
    RootRow { title: "Custom buttons", subtitle: "Execute lua code with custom buttons", icon: "utilities-terminal-symbolic", target: "settings_player_custom_buttons" },
    RootRow { title: "Script editor", subtitle: "Edit and create new scripts and script options", icon: "text-editor-symbolic", target: "settings_player_editor" },
    RootRow { title: "Torrent", subtitle: "TorrServer options", icon: "network-workgroup-symbolic", target: "settings_player_torrents" },
    RootRow { title: "Advanced", subtitle: "User files, mpv.conf, input.conf", icon: "applications-engineering-symbolic", target: "settings_player_advanced" },
];

/// The root list as a preference tree: one plain row per entry (navigation is the caller's job via `target`).
pub fn root_rows() -> Vec<Preference> {
    let rows: Vec<PreferenceItem> = ROOT_ROWS
        .iter()
        .map(|row| {
            let mut item = text(row.title, Some(row.subtitle), true);
            if let PreferenceItem::Text { common, .. } = &mut item {
                common.icon = Some(row.icon);
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
        let tree = root_rows();
        let Preference::Group(g) = &tree[0] else { panic!("group") };
        assert_eq!(g.items.len(), 9);
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
