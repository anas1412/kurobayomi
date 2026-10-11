//! Player settings. The root page holds the kept settings (the subtitle and audio languages, the default volume, autoplay
//! and the mini player) and the links to the pages that stay: Custom buttons, Script editor and
//! Torrent. The Internal player, Gestures, Decoder, Subtitles, Audio and Advanced pages are cut by the settings diet.
pub mod conf_edit;
pub mod controls;
pub mod files;
pub mod highlight;
pub mod language;
pub mod root;
pub mod scripts;
pub mod syntax;
pub mod torrent;

use std::rc::Rc;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::rows::{group, text};

/// One link row of the root page: the sub-screen it opens is `target`, a settings screen id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootRow {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub icon: &'static str,
    pub target: &'static str,
}

/// The links that stay on the root page, in order.
pub const LINK_ROWS: [RootRow; 3] = [
    RootRow { title: "Custom buttons", subtitle: "Execute lua code with custom buttons", icon: "utilities-terminal-symbolic", target: "settings_player_custom_buttons" },
    RootRow { title: "Script editor", subtitle: "Edit and create new scripts and script options", icon: "document-edit-symbolic", target: "settings_player_editor" },
    RootRow { title: "Torrent", subtitle: "TorrServer options", icon: "network-transmit-receive-symbolic", target: "settings_player_torrents" },
];

/// The link rows as one group; a tap runs `open` with the row's `target`.
pub fn link_rows(open: Rc<dyn Fn(&'static str)>) -> Vec<Preference> {
    let rows: Vec<PreferenceItem> = LINK_ROWS
        .iter()
        .map(|row| {
            let mut item = text(row.title, Some(row.subtitle), true);
            if let PreferenceItem::Text { common, on_click } = &mut item {
                common.icon = Some(row.icon);
                common.link = true;
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
    fn three_link_rows_with_the_spec_targets_in_order() {
        let targets: Vec<&str> = LINK_ROWS.iter().map(|r| r.target).collect();
        assert_eq!(targets, ["settings_player_custom_buttons", "settings_player_editor", "settings_player_torrents"]);
        let tree = link_rows(Rc::new(|_| {}));
        let Preference::Group(g) = &tree[0] else { panic!("group") };
        assert_eq!(g.items.len(), 3);
    }

    #[test]
    fn tapping_a_link_opens_its_target() {
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let tree = link_rows(Rc::new(move |target| sink.borrow_mut().push(target)));
        let Preference::Group(g) = &tree[0] else { panic!("group") };
        for item in &g.items {
            let PreferenceItem::Text { on_click, .. } = item else { panic!("text row") };
            (on_click.as_ref().expect("a tap handler"))();
        }
        let targets: Vec<&str> = LINK_ROWS.iter().map(|r| r.target).collect();
        assert_eq!(*seen.borrow(), targets);
    }

    #[test]
    fn every_link_target_is_a_spec_screen_and_the_icons_follow_the_spec_note() {
        crate::ui::with_spec(|spec| {
            for row in LINK_ROWS {
                assert!(spec.screen(row.target).is_some(), "{}", row.target);
            }
        });
        let icons: Vec<&str> = LINK_ROWS.iter().map(|r| r.icon).collect();
        assert_eq!(icons[0], "utilities-terminal-symbolic", "terminal");
        assert_eq!(icons[1], "document-edit-symbolic", "edit-note");
    }
}
